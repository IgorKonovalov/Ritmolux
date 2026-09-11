//! The analytic field: one fullscreen fragment pass whose output is a closed-form
//! function of position, coloured through the shared palette LUT (ADR-0021).
//!
//! Every stateless per-pixel mathematical world lives here as a named
//! **family** (ADR-0180 rule 1), selected by `[field] family`. The first family
//! is the Chladni plate, whose two integer mode numbers are the whole figure.
//!
//! # No state between frames
//!
//! Nothing here accumulates: the picture is a pure function of the parameters
//! the preset binds this frame, the palette, and the target's aspect. So there
//! is no `Phase`, no offscreen of its own, and nothing for a capture to reset —
//! motion comes from what the bindings read (`time`, the bands), never from the
//! scene.
//!
//! # The aspect is the render target's
//!
//! ADR-0037's rule, restated because a fullscreen field computing a radius is
//! exactly the shape that gets it wrong: the field's x axis is scaled by the
//! `aspect` [`Scene::render`] is handed, which is the target's, so one field
//! unit is the same number of pixels on both axes and a square plate stays
//! square at 1280x800.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

mod shader;

use super::common;
use super::lines::GeneratorConfig;
use super::{FamilyParam, FamilyRange, Scene};
use crate::dsp::AnalysisFrame;
use crate::render::gpu;
use crate::render::palette::{self, Palette};
use crate::render::scenes::{ParamKind, ParamSpec, default_of};

/// Which closed-form world the field draws (ADR-0180 rule 1).
///
/// Quasicrystal, Voronoi and hyperbolic tiling are placed in this system by
/// that rule and are not built; each is a later arm here, not a new system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldFamily {
    /// The cymatics plate: the nodal set of two standing waves whose mode
    /// numbers are `mode_n` and `mode_m`.
    #[default]
    Chladni,
}

impl FieldFamily {
    /// Every family, in the order the shader's family index numbers them and
    /// the generated reference lists them.
    pub const ALL: [FieldFamily; 1] = [FieldFamily::Chladni];

    /// Parse the canonical `[field] family = "..."` value, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.as_str() == name)
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            FieldFamily::Chladni => "chladni",
        }
    }

    /// The integer the shader's `switch` selects on. Its position in
    /// [`ALL`](Self::ALL), which is what keeps the two numbered alike.
    fn index(self) -> u32 {
        match self {
            FieldFamily::Chladni => 0,
        }
    }
}

/// `[field]` — the structural configuration, fixed while the preset is loaded
/// and delivered through `Scene::configure`, in the shape `[curve]` and
/// `[particles]` already use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldConfig {
    /// Which world is drawn.
    pub family: FieldFamily,
}

/// The largest mode number either axis of the plate accepts. Past it the nodal
/// lines are closer together than a 1080p frame has pixels to separate them.
pub const MAX_MODE: f32 = 16.0;

/// A bound mode number as the plate reads it: clamped into `1..=MAX_MODE` and
/// rounded, so the figure is always one of the plate's own standing waves and
/// never a fractional mode — which is not a figure at all, since the boundary
/// condition the formula encodes holds only at whole numbers. A non-finite value
/// falls back to the declared default rather than reaching the shader.
pub(crate) fn applied_mode(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(1.0, MAX_MODE).round()
    } else {
        default
    }
}

const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_COLOR_SPAN: f32 = default_of(PARAMS, "color_span");
const DEFAULT_COLOR_CENTER: f32 = default_of(PARAMS, "color_center");
const DEFAULT_MODE_N: f32 = default_of(PARAMS, "mode_n");
const DEFAULT_MODE_M: f32 = default_of(PARAMS, "mode_m");
const DEFAULT_LINE_WIDTH: f32 = default_of(PARAMS, "line_width");
const DEFAULT_PLATE_MIX: f32 = default_of(PARAMS, "plate_mix");

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/preset.rs` fails if the two
/// drift, and [`FAMILY_PARAMS`] says which family reads each family-bound one.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "mode_n",
        default: 3.0,
        range: Some([1.0, MAX_MODE]),
        doc: "The plate's first mode number: how many nodal lines cross one axis. Equal to \
              `mode_m`, the two waves cancel and the plate is blank.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "mode_m",
        default: 5.0,
        range: Some([1.0, MAX_MODE]),
        doc: "The plate's second mode number: how many nodal lines cross the other axis.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "line_width",
        default: 0.04,
        range: Some([0.0, 0.3]),
        doc: "How wide a band around the nodal lines lights, in plate units (the plate is 2 \
              across); 0 is a one-pixel line.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "plate_mix",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Blends from the nodal lines alone toward the whole signed wave, which reads as a \
              standing wave rather than as sand.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "color_span",
        default: 1.0,
        range: Some([0.0, 4.0]),
        doc: "How much of the palette the field's level covers; 0 is one flat colour.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "color_center",
        default: 0.0,
        range: Some([-1.0, 1.0]),
        doc: "Shifts which part of the palette the field's level starts from.",
        kind: ParamKind::Modal,
    },
    common::brightness(common::DEFAULT_BRIGHTNESS),
    common::hue(DEFAULT_HUE),
    common::zoom(DEFAULT_ZOOM),
    common::PAN_X,
    common::PAN_Y,
    common::SATURATION,
    common::PALETTE_MIX,
    common::PALETTE_STEPS,
    common::PALETTE_CONTOUR,
];

/// One row of [`FAMILY_PARAMS`], its ranges in [`FieldFamily::ALL`]'s order.
/// `None` is a family that does not read the parameter.
macro_rules! per_family {
    ($name:literal: $chladni:expr $(,)?) => {
        FamilyParam {
            name: $name,
            ranges: &[FamilyRange {
                family: "chladni",
                range: $chladni,
            }],
        }
    };
}

/// Every parameter only one family reads (ADR-0180 rule 4), with the range that
/// reads there — so the generated reference prints `mode_n` as Chladni's and
/// inert elsewhere, rather than leaving an author to find it dead. Ranges are in
/// [`FieldFamily::ALL`]'s order, and each row's spec range is one of them; both
/// held by this module's tests. A parameter missing from here reads the same on
/// every family.
pub const FAMILY_PARAMS: &[FamilyParam] = &[
    per_family!("mode_n": Some([1.0, MAX_MODE])),
    per_family!("mode_m": Some([1.0, MAX_MODE])),
    per_family!("line_width": Some([0.0, 0.3])),
    per_family!("plate_mix": Some([0.0, 1.0])),
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
    e: [f32; 4],
}

/// The fullscreen analytic field, driven by named preset parameters and one
/// `[field]` table.
pub struct AnalyticFieldScene {
    /// The pipeline, the uniform buffer, the A/B gradient LUT pair (ADR-0021)
    /// and the one bind group that binds all four.
    gpu: gpu::FullscreenScene,
    /// The `[field]` table of the preset last configured.
    config: FieldConfig,
    /// Height of the target this scene renders into this frame, in pixels —
    /// what turns "one pixel" into field units for the line's antialiasing.
    target_height: u32,
    colour: common::PaletteParams,
    pan: common::PanParams,
    zoom: f32,
    color_span: f32,
    color_center: f32,
    mode_n: f32,
    mode_m: f32,
    line_width: f32,
    plate_mix: f32,
    /// How much of this field's coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame through
    /// [`Scene::set_occlude`] — **not** a named param, so `reset_params` leaves
    /// it alone.
    occlude: f32,
}

impl AnalyticFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = gpu::fullscreen_shader(
            device,
            "analytic-field-shader",
            gpu::FULLSCREEN_VS_NDC,
            shader::SHADER,
        );
        let parts =
            gpu::FullscreenParts::new(device, "analytic-field", std::mem::size_of::<Params>());
        // `[Texture, Texture, Sampler, Uniform]` in one group — a shape the
        // crate's layout enumeration shows nothing else has (ADR-0058). See the
        // WGSL note for why this scene does not split them over two groups.
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("analytic-field-bind-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::texture(1, true),
                gpu::sampler(2),
                gpu::uniform(3, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let [lut_a, lut_b, sampler] = parts.luts().bind_entries(0, 1, 2);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("analytic-field-bind-group"),
            layout: &layout,
            entries: &[
                lut_a,
                lut_b,
                sampler,
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: parts.uniforms().as_entire_binding(),
                },
            ],
        });

        Self {
            gpu: parts.finish(
                device,
                &shader,
                &[&layout],
                bind_group,
                None,
                surface_format,
                wgpu::BlendState::REPLACE,
                "analytic-field",
            ),
            config: FieldConfig::default(),
            target_height: 1,
            colour: common::PaletteParams::new(DEFAULT_HUE, common::DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            zoom: DEFAULT_ZOOM,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            mode_n: DEFAULT_MODE_N,
            mode_m: DEFAULT_MODE_M,
            line_width: DEFAULT_LINE_WIDTH,
            plate_mix: DEFAULT_PLATE_MIX,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
        }
    }
}

impl Scene for AnalyticFieldScene {
    fn name(&self) -> &'static str {
        "analytic field"
    }

    fn set_target_size(&mut self, _width: u32, height: u32) {
        self.target_height = height.max(1);
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        // Stored here, uploaded by `render` — off the hot path, once a switch.
        self.gpu.set_palette(palette);
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<super::CapOverflow> {
        if let GeneratorConfig::Field(config) = cfg {
            self.config = *config;
        }
        None
    }

    fn reset_params(&mut self) {
        self.colour.reset();
        self.pan.reset();
        self.zoom = DEFAULT_ZOOM;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.mode_n = DEFAULT_MODE_N;
        self.mode_m = DEFAULT_MODE_M;
        self.line_width = DEFAULT_LINE_WIDTH;
        self.plate_mix = DEFAULT_PLATE_MIX;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "zoom" => self.zoom = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "mode_n" => self.mode_n = value,
            "mode_m" => self.mode_m = value,
            "line_width" => self.line_width = value,
            "plate_mix" => self.plate_mix = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Stateless: the analysis reaches this scene only through the bindings.
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        self.gpu.flush_palette(queue);

        let zoom = if self.zoom.is_finite() && self.zoom > 1e-3 {
            self.zoom
        } else {
            DEFAULT_ZOOM
        };
        // One pixel of the target, in field units: the short axis spans 2 / zoom.
        let pixel = 2.0 / (zoom * self.target_height as f32);
        let params = Params {
            a: [aspect.max(0.1), zoom, self.pan.x, self.pan.y],
            b: [
                self.colour.hue,
                self.color_span,
                self.color_center,
                self.colour.saturation,
            ],
            c: [
                self.colour.mix,
                self.occlude,
                palette::band_steps(self.colour.steps),
                palette::band_contour(self.colour.contour),
            ],
            d: [
                self.colour.brightness,
                self.config.family.index() as f32,
                applied_mode(self.mode_n, DEFAULT_MODE_N),
                applied_mode(self.mode_m, DEFAULT_MODE_M),
            ],
            e: [self.line_width, self.plate_mix, pixel, 0.0],
        };
        self.gpu.write_uniform(queue, &params);

        // Load over the engine backdrop (ADR-0018); the field covers every pixel
        // and says how much through alpha.
        self.gpu
            .draw(encoder, "analytic-field-pass", view, wgpu::LoadOp::Load);
    }
}

#[cfg(test)]
mod tests;
