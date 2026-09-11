//! The analytic field: one fullscreen fragment pass whose output is a closed-form
//! function of position, coloured through the shared palette LUT (ADR-0021).
//!
//! Every stateless per-pixel mathematical world lives here as a named
//! **family** (ADR-0180 rule 1), selected by `[field] family`: the Chladni
//! plate, whose two integer mode numbers are the whole figure, and escape time
//! — the Julia and Mandelbrot sets — coloured by the smooth iteration count.
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
    /// Escape time: `z -> z^power + c` iterated to an escape radius, coloured
    /// by the smooth iteration count — the Julia and Mandelbrot sets.
    EscapeTime,
}

impl FieldFamily {
    /// Every family, in the order the shader's family index numbers them and
    /// the generated reference lists them.
    pub const ALL: [FieldFamily; 2] = [FieldFamily::Chladni, FieldFamily::EscapeTime];

    /// Parse the canonical `[field] family = "..."` value, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.as_str() == name)
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            FieldFamily::Chladni => "chladni",
            FieldFamily::EscapeTime => "escape_time",
        }
    }

    /// The integer the shader's `switch` selects on. Its position in
    /// [`ALL`](Self::ALL), which is what keeps the two numbered alike.
    fn index(self) -> u32 {
        match self {
            FieldFamily::Chladni => 0,
            FieldFamily::EscapeTime => 1,
        }
    }
}

/// Escape time only: what the pixel is (ADR-0180's `[field] map`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EscapeMap {
    /// The pixel seeds the orbit and `c` is the `c_re`/`c_im` constant — one
    /// Julia set, which the constant reshapes.
    #[default]
    Julia,
    /// The pixel is `c` and every orbit starts at zero — the Mandelbrot set,
    /// the map of which constants give a connected Julia set. `c_re`/`c_im` are
    /// inert.
    Mandelbrot,
}

impl EscapeMap {
    /// Both maps, for the load error and the schema export.
    pub const ALL: [EscapeMap; 2] = [EscapeMap::Julia, EscapeMap::Mandelbrot];

    /// Parse the canonical `[field] map = "..."` value, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|map| map.as_str() == name)
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            EscapeMap::Julia => "julia",
            EscapeMap::Mandelbrot => "mandelbrot",
        }
    }
}

/// Escape time only: the shape an orbit is measured against (`[field] trap`).
///
/// With a trap, a pixel is coloured by the **smallest distance its orbit came
/// to the shape** rather than by how fast it escaped — which is what draws the
/// filaments, rings and stained-glass cells of orbit-trap imagery. Each shape
/// sits `trap_radius` from the origin, turned by `trap_rotate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrapShape {
    /// No trap: the smooth escape count colours the picture.
    #[default]
    None,
    /// A point `trap_radius` from the origin along `trap_rotate`.
    Point,
    /// A line `trap_radius` from the origin, its normal along `trap_rotate`.
    Line,
    /// Two perpendicular lines crossing at the point `Point` would sit at,
    /// turned by `trap_rotate`.
    Cross,
    /// A circle of radius `trap_radius` about the origin; `trap_rotate` is inert
    /// on it.
    Circle,
}

impl TrapShape {
    /// Every shape, in the order the shader's trap index numbers them.
    pub const ALL: [TrapShape; 5] = [
        TrapShape::None,
        TrapShape::Point,
        TrapShape::Line,
        TrapShape::Cross,
        TrapShape::Circle,
    ];

    /// Parse the canonical `[field] trap = "..."` value, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|trap| trap.as_str() == name)
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            TrapShape::None => "none",
            TrapShape::Point => "point",
            TrapShape::Line => "line",
            TrapShape::Cross => "cross",
            TrapShape::Circle => "circle",
        }
    }

    /// The integer the shader's trap `switch` selects on: its position in
    /// [`ALL`](Self::ALL), `0` being no trap at all.
    fn index(self) -> u32 {
        match self {
            TrapShape::None => 0,
            TrapShape::Point => 1,
            TrapShape::Line => 2,
            TrapShape::Cross => 3,
            TrapShape::Circle => 4,
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
    /// Escape time only: whether `c` is a parameter or the pixel. The loader
    /// refuses a `map` on any other family.
    pub map: EscapeMap,
    /// Escape time only: the orbit trap, if any. The loader refuses a `trap`
    /// on any other family.
    pub trap: TrapShape,
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

/// The most iterations any tier lets an escape-time preset ask for — the top of
/// `iterations`' declared range, and the loop bound the shader is handed.
pub const MAX_ITERATIONS: f32 = 512.0;

/// A bound iteration budget as the shader reads it: clamped into
/// `1..=cap` and rounded, so the loop bound is a whole number the tier allows.
/// A non-finite value falls back to the declared default, itself capped.
pub(crate) fn applied_iterations(value: f32, cap: f32) -> f32 {
    let cap = cap.clamp(1.0, MAX_ITERATIONS);
    if value.is_finite() {
        value.clamp(1.0, cap).round()
    } else {
        DEFAULT_ITERATIONS.min(cap)
    }
}

/// The clamp to report for a bound budget of `value` under `cap`, or `None`
/// when the preset asked within it — or drew a family that reads no budget,
/// where there is nothing to clamp. `value` is compared as the shader would
/// take it before the cap, so a bound 64.4 under a cap of 64 is not a clamp.
pub(crate) fn iteration_clamp(
    family: FieldFamily,
    value: f32,
    cap: f32,
) -> Option<super::CapOverflow> {
    if family != FieldFamily::EscapeTime {
        return None;
    }
    let asked = applied_iterations(value, MAX_ITERATIONS);
    let applied = applied_iterations(value, cap);
    (asked > applied).then_some(super::CapOverflow {
        dropped: (asked - applied) as usize,
        context: super::OverflowContext::Iterations(asked as u32),
        cap: applied as usize,
    })
}

/// `value` clamped into `[lo, hi]`, or `fallback` when it is not finite. The
/// escape-time parameters are clamped CPU-side because the shader's finiteness
/// argument rests on their bounds (see its `escape_time` comment): a power at 1
/// divides by `ln 1`, a radius under 2 lets bounded orbits "escape", and an
/// unbounded `c` can overflow f32 in one step.
fn bounded(value: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

/// The smallest `power` the shader is handed: the smooth count divides by
/// `ln power`, which vanishes at 1.
pub const MIN_POWER: f32 = 1.1;
/// The largest `power`, which with the largest radius keeps `|z|^power` inside
/// f32 for one step past escape.
pub const MAX_POWER: f32 = 8.0;
/// The escape radius's clamp. Below 2 an orbit that stays bounded can still
/// cross it; above 256 one step past it can overflow at the largest power.
pub const ESCAPE_RADIUS_RANGE: [f32; 2] = [2.0, 256.0];
/// How far `c` may sit from the origin on either axis. Every constant with a
/// connected Julia set lies inside `|c| <= 2`; this leaves room for a binding
/// to overshoot into dust without letting it reach f32's ceiling.
pub const C_LIMIT: f32 = 4.0;

const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_COLOR_SPAN: f32 = default_of(PARAMS, "color_span");
const DEFAULT_COLOR_CENTER: f32 = default_of(PARAMS, "color_center");
const DEFAULT_MODE_N: f32 = default_of(PARAMS, "mode_n");
const DEFAULT_MODE_M: f32 = default_of(PARAMS, "mode_m");
const DEFAULT_LINE_WIDTH: f32 = default_of(PARAMS, "line_width");
const DEFAULT_PLATE_MIX: f32 = default_of(PARAMS, "plate_mix");
const DEFAULT_C_RE: f32 = default_of(PARAMS, "c_re");
const DEFAULT_C_IM: f32 = default_of(PARAMS, "c_im");
const DEFAULT_ITERATIONS: f32 = default_of(PARAMS, "iterations");
const DEFAULT_ESCAPE_RADIUS: f32 = default_of(PARAMS, "escape_radius");
const DEFAULT_POWER: f32 = default_of(PARAMS, "power");
const DEFAULT_INTERIOR: f32 = default_of(PARAMS, "interior");
const DEFAULT_TRAP_RADIUS: f32 = default_of(PARAMS, "trap_radius");
const DEFAULT_TRAP_ROTATE: f32 = default_of(PARAMS, "trap_rotate");

/// How far from the origin a trap may sit, and how far `trap_radius` may push
/// it — past every orbit's escape radius a trap is simply never approached.
pub const TRAP_RADIUS_LIMIT: f32 = 256.0;

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
        name: "iterations",
        default: 64.0,
        range: Some([1.0, MAX_ITERATIONS]),
        doc: "How many steps an orbit is followed before it is called part of the set; more \
              resolves finer boundary detail. Capped by the quality tier.",
        kind: ParamKind::Structural,
    },
    ParamSpec {
        name: "c_re",
        default: -0.8,
        range: Some([-1.5, 0.5]),
        doc: "The real part of the Julia constant: the lever that reshapes the set, from one \
              connected piece to dust. Inert on the `mandelbrot` map.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "c_im",
        default: 0.156,
        range: Some([-1.0, 1.0]),
        doc: "The imaginary part of the Julia constant. Inert on the `mandelbrot` map.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "escape_radius",
        default: 16.0,
        range: Some([2.0, 256.0]),
        doc: "How far an orbit must travel to count as escaped; larger smooths the colour \
              bands' spacing.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "power",
        default: 2.0,
        range: Some([1.5, MAX_POWER]),
        doc: "The exponent in z -> z^power + c: 2 is the classic set, higher whole powers add \
              lobes, and a fractional power tears along the negative real axis.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "interior",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "How much light the set itself emits; 0 is the textbook black interior.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "trap_radius",
        default: 0.5,
        range: Some([0.0, 2.0]),
        doc: "How far the orbit trap sits from the origin — the circle's radius, the line's \
              offset, the point's and the cross's distance. Inert with no `trap`.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "trap_rotate",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Turns the orbit trap about the origin, in whole turns. Inert on a `circle` and \
              with no `trap`.",
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
    ($name:literal: $chladni:expr, $escape_time:expr $(,)?) => {
        FamilyParam {
            name: $name,
            ranges: &[
                FamilyRange {
                    family: "chladni",
                    range: $chladni,
                },
                FamilyRange {
                    family: "escape_time",
                    range: $escape_time,
                },
            ],
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
    per_family!("mode_n": Some([1.0, MAX_MODE]), None),
    per_family!("mode_m": Some([1.0, MAX_MODE]), None),
    per_family!("line_width": Some([0.0, 0.3]), None),
    per_family!("plate_mix": Some([0.0, 1.0]), None),
    per_family!("iterations": None, Some([1.0, MAX_ITERATIONS])),
    per_family!("c_re": None, Some([-1.5, 0.5])),
    per_family!("c_im": None, Some([-1.0, 1.0])),
    per_family!("escape_radius": None, Some([2.0, 256.0])),
    per_family!("power": None, Some([1.5, MAX_POWER])),
    per_family!("interior": None, Some([0.0, 1.0])),
    per_family!("trap_radius": None, Some([0.0, 2.0])),
    per_family!("trap_rotate": None, Some([0.0, 1.0])),
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
    e: [f32; 4],
    f: [f32; 4],
    g: [f32; 4],
    h: [f32; 4],
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
    /// The tier's [`field_iterations`](crate::render::TierConfig::field_iterations):
    /// the most iterations a bound budget reaches the shader with.
    iteration_cap: f32,
    /// This frame's clamp of the budget to [`iteration_cap`](Self::iteration_cap),
    /// or `None` when the preset asked within it — read back through
    /// [`Scene::mirror_overflow`] so the frontend announces the clamp. A `Copy`
    /// value rewritten each frame, so reporting it allocates nothing.
    clamp: Option<super::CapOverflow>,
    colour: common::PaletteParams,
    pan: common::PanParams,
    zoom: f32,
    color_span: f32,
    color_center: f32,
    mode_n: f32,
    mode_m: f32,
    line_width: f32,
    plate_mix: f32,
    c_re: f32,
    c_im: f32,
    iterations: f32,
    escape_radius: f32,
    power: f32,
    interior: f32,
    trap_radius: f32,
    trap_rotate: f32,
    /// How much of this field's coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame through
    /// [`Scene::set_occlude`] — **not** a named param, so `reset_params` leaves
    /// it alone.
    occlude: f32,
}

impl AnalyticFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`, holding a
    /// bound escape-time budget to `iteration_cap` — the tier's
    /// [`field_iterations`](crate::render::TierConfig::field_iterations).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        iteration_cap: u32,
    ) -> Self {
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
            iteration_cap: (iteration_cap as f32).clamp(1.0, MAX_ITERATIONS),
            clamp: None,
            colour: common::PaletteParams::new(DEFAULT_HUE, common::DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            zoom: DEFAULT_ZOOM,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            mode_n: DEFAULT_MODE_N,
            mode_m: DEFAULT_MODE_M,
            line_width: DEFAULT_LINE_WIDTH,
            plate_mix: DEFAULT_PLATE_MIX,
            c_re: DEFAULT_C_RE,
            c_im: DEFAULT_C_IM,
            iterations: DEFAULT_ITERATIONS,
            escape_radius: DEFAULT_ESCAPE_RADIUS,
            power: DEFAULT_POWER,
            interior: DEFAULT_INTERIOR,
            trap_radius: DEFAULT_TRAP_RADIUS,
            trap_rotate: DEFAULT_TRAP_ROTATE,
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
            // The outgoing preset's clamp is not this one's to report.
            self.clamp = None;
        }
        None
    }

    fn mirror_overflow(&self) -> Option<&super::CapOverflow> {
        self.clamp.as_ref()
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
        self.c_re = DEFAULT_C_RE;
        self.c_im = DEFAULT_C_IM;
        self.iterations = DEFAULT_ITERATIONS;
        self.escape_radius = DEFAULT_ESCAPE_RADIUS;
        self.power = DEFAULT_POWER;
        self.interior = DEFAULT_INTERIOR;
        self.trap_radius = DEFAULT_TRAP_RADIUS;
        self.trap_rotate = DEFAULT_TRAP_ROTATE;
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
            "c_re" => self.c_re = value,
            "c_im" => self.c_im = value,
            "iterations" => self.iterations = value,
            "escape_radius" => self.escape_radius = value,
            "power" => self.power = value,
            "interior" => self.interior = value,
            "trap_radius" => self.trap_radius = value,
            "trap_rotate" => self.trap_rotate = value,
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
        // A pan is unbounded but must be a number: the escape arm clamps the
        // point it seeds an orbit with, and a clamp cannot rescue a NaN.
        let pan = |v: f32| if v.is_finite() { v } else { 0.0 };
        self.clamp = iteration_clamp(self.config.family, self.iterations, self.iteration_cap);
        let params = Params {
            a: [aspect.max(0.1), zoom, pan(self.pan.x), pan(self.pan.y)],
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
            f: [
                bounded(self.c_re, -C_LIMIT, C_LIMIT, DEFAULT_C_RE),
                bounded(self.c_im, -C_LIMIT, C_LIMIT, DEFAULT_C_IM),
                applied_iterations(self.iterations, self.iteration_cap),
                bounded(
                    self.escape_radius,
                    ESCAPE_RADIUS_RANGE[0],
                    ESCAPE_RADIUS_RANGE[1],
                    DEFAULT_ESCAPE_RADIUS,
                ),
            ],
            g: [
                bounded(self.power, MIN_POWER, MAX_POWER, DEFAULT_POWER),
                bounded(self.interior, 0.0, 1.0, DEFAULT_INTERIOR),
                match self.config.map {
                    EscapeMap::Julia => 0.0,
                    EscapeMap::Mandelbrot => 1.0,
                },
                0.0,
            ],
            h: [
                self.config.trap.index() as f32,
                bounded(
                    self.trap_radius,
                    -TRAP_RADIUS_LIMIT,
                    TRAP_RADIUS_LIMIT,
                    DEFAULT_TRAP_RADIUS,
                ),
                // Whole turns to radians; `fract` keeps a long-running bound
                // angle from losing precision without moving the picture.
                std::f32::consts::TAU * bounded(self.trap_rotate, -1e6, 1e6, 0.0).fract(),
                0.0,
            ],
        };
        self.gpu.write_uniform(queue, &params);

        // Load over the engine backdrop (ADR-0018); the field covers every pixel
        // and says how much through alpha.
        self.gpu
            .draw(encoder, "analytic-field-pass", view, wgpu::LoadOp::Load);
    }
}

#[cfg(test)]
mod mirror;
#[cfg(test)]
mod tests;
