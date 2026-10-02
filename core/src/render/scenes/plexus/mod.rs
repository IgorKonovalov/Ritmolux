//! The plexus system (ADR-0257): a few hundred points in 3D, joined by a line
//! wherever two lie within a bindable link distance, seen through the shared
//! perspective camera.
//!
//! # One frame
//!
//! `update` advances the point set by the injected `dt` and rebuilds the
//! proximity graph from it (`sim`): every pair closer than `link_distance` is
//! an edge whose presence is `smoothstep(1 - d / link_distance)`, so a link
//! fades in as its pair nears and out as it parts. `render` builds this frame's
//! [`Camera3d`] view from the render target's aspect (ADR-0037), clips each edge
//! against the near plane, drops the ones wholly off one edge of the frame,
//! colours each by its depth, and hands the rest to the shared line renderer's
//! `seg3d` pipeline. A node is drawn at every point through the 3D sprite
//! pipeline beside the marks (`marks::InstancedQuads3d`), coloured the same way
//! and blurred by the same `coc()`.
//!
//! # What is structural and what is bound
//!
//! The `[plexus]` table — the layout, the point count and the seed — is fixed
//! for as long as the preset is loaded, because it decides how many points
//! exist and where they start. Everything that moves is a bindable parameter.
//!
//! # Buffers are sized once
//!
//! The point set, the edge list and the instance scratch are reserved from the
//! tier's caps when the scene is built, and a preset asking for more is held to
//! them at load, so no frame allocates.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set).
// `update` and `render` run every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub(crate) mod sim;

use crate::dsp::AnalysisFrame;
use crate::render::camera::{self, CameraParams};
use crate::render::palette::{self, Palette};
use crate::render::scenes::common::{PaletteParams, PanParams};
use crate::render::scenes::lines::{GeneratorConfig, LineRenderer, Segment3dInstance};
use crate::render::scenes::marks::{InstancedQuads3d, Quad3dInstance};
use crate::render::scenes::{
    FamilyParam, FamilyRange, ParamGroup, ParamKind, ParamSpec, Scene, default_of,
};

/// Which arrangement the points take — the `[plexus] layout` family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlexusLayout {
    /// Points scattered through a cube, drifting on a seeded flow.
    #[default]
    Cloud,
    /// A jittered grid on a plane, rippled along its normal by a seeded height
    /// field.
    Sheet,
}

impl PlexusLayout {
    /// Every layout, in roster order — the closed set, and the list the schema
    /// export renders rather than restating.
    pub const ALL: [PlexusLayout; 2] = [PlexusLayout::Cloud, PlexusLayout::Sheet];

    /// Parse a `[plexus] layout` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.as_str() == name)
    }

    /// The `[plexus] layout` name this parses from.
    pub fn as_str(self) -> &'static str {
        match self {
            PlexusLayout::Cloud => "cloud",
            PlexusLayout::Sheet => "sheet",
        }
    }
}

/// The fewest points a preset may ask for. Below this the graph is a handful of
/// strokes rather than a network.
pub const MIN_POINTS: u32 = 16;

/// The most points a preset may ask for at load, before the tier's own cap
/// holds it further. The pairing is quadratic, so this bounds the worst frame
/// on any tier.
pub const MAX_POINTS: u32 = 4096;

/// The point count a preset that names none draws.
pub const DEFAULT_POINTS: u32 = 300;

/// The `[plexus]` table, validated (ADR-0180 rule 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlexusConfig {
    /// Which arrangement the points take.
    pub layout: PlexusLayout,
    /// How many points, in [`MIN_POINTS`]`..=`[`MAX_POINTS`]; the tier may hold
    /// it lower.
    pub points: u32,
    /// What every point's start and the flow are drawn from.
    pub seed: u64,
}

impl Default for PlexusConfig {
    fn default() -> Self {
        Self {
            layout: PlexusLayout::default(),
            points: DEFAULT_POINTS,
            seed: 0,
        }
    }
}

/// The stroke profile every plexus edge is drawn with: a solid core over the
/// inner half and a ramp across the outer (ADR-0124). Fixed rather than bound,
/// since the look this system is for is a fine luminous line.
const SOFTNESS: f32 = 0.5;

const DEFAULT_LINK_DISTANCE: f32 = default_of(PARAMS, "link_distance");
const DEFAULT_LINK_ALPHA: f32 = default_of(PARAMS, "link_alpha");
const DEFAULT_LINE_WIDTH: f32 = default_of(PARAMS, "line_width");
const DEFAULT_DRIFT: f32 = default_of(PARAMS, "drift");
const DEFAULT_WAVE: f32 = default_of(PARAMS, "wave");
const DEFAULT_WAVE_SCALE: f32 = default_of(PARAMS, "wave_scale");
const DEFAULT_NODE_SIZE: f32 = default_of(PARAMS, "node_size");
const DEFAULT_NODE_GLOW: f32 = default_of(PARAMS, "node_glow");
const DEFAULT_BRIGHTNESS: f32 = default_of(PARAMS, "brightness");
const DEFAULT_HUE_CENTER: f32 = default_of(PARAMS, "hue_center");
const DEFAULT_HUE_SPREAD: f32 = default_of(PARAMS, "hue_spread");
const DEFAULT_ZOOM: f32 = default_of(PARAMS, "zoom");

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "link_distance",
        default: 0.35,
        range: Some([0.05, 1.0]),
        doc: "How close two points must be to be joined, in the layout's own units; the cube is 2 across.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "link_alpha",
        default: 0.7,
        range: Some([0.0, 1.0]),
        doc: "How strongly a link at its closest is drawn; a link always fades to nothing at link_distance.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: false,
    },
    ParamSpec {
        name: "line_width",
        default: 1.5,
        range: Some([0.5, 8.0]),
        doc: "Line width in pixels at the focal plane; nearer lines are wider and farther ones thinner.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "node_size",
        default: 2.5,
        range: Some([0.0, 12.0]),
        doc: "Radius of the dot at every point, in pixels at the focal plane; 0 draws no dots.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "node_glow",
        default: 1.0,
        range: Some([0.0, 4.0]),
        doc: "Brightness of the dots relative to the lines.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: false,
    },
    ParamSpec {
        name: "drift",
        default: 0.15,
        range: Some([0.0, 1.0]),
        doc: "How fast the points drift on their flow; 0 holds the network still.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: true,
    },
    ParamSpec {
        name: "wave",
        default: 0.15,
        range: Some([0.0, 0.6]),
        doc: "How far a sheet ripples above and below its plane, in the layout's own units; 0 lies flat.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "wave_scale",
        default: 1.0,
        range: Some([0.3, 3.0]),
        doc: "How broad a sheet's ripples are; larger is a slower swell, smaller a fine chop.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: false,
    },
    camera::YAW,
    camera::PITCH,
    camera::DISTANCE,
    camera::FOV,
    camera::FOCUS,
    camera::APERTURE,
    crate::render::scenes::common::brightness(1.0),
    ParamSpec {
        name: "hue_center",
        default: 0.5,
        range: Some([0.0, 1.0]),
        doc: "Where along the palette the middle of the volume's depth is coloured.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    ParamSpec {
        name: "hue_spread",
        default: 0.5,
        range: Some([0.0, 1.0]),
        doc: "How far along the palette the colour travels from the nearest part of the volume to the farthest.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    crate::render::scenes::common::SATURATION,
    crate::render::scenes::common::PALETTE_MIX,
    crate::render::scenes::common::PALETTE_STEPS,
    crate::render::scenes::common::zoom(1.0),
    crate::render::scenes::common::PAN_X,
    crate::render::scenes::common::PAN_Y,
];

/// Every parameter only some layouts read (ADR-0180 rule 4), with the range
/// that reads there. A parameter missing from here reads the same on both.
pub const FAMILY_PARAMS: &[FamilyParam] = &[
    FamilyParam {
        name: "wave",
        ranges: &[
            FamilyRange {
                family: "cloud",
                range: None,
            },
            FamilyRange {
                family: "sheet",
                range: Some([0.0, 0.6]),
            },
        ],
    },
    FamilyParam {
        name: "wave_scale",
        ranges: &[
            FamilyRange {
                family: "cloud",
                range: None,
            },
            FamilyRange {
                family: "sheet",
                range: Some([0.3, 3.0]),
            },
        ],
    },
];

/// The plexus scene: the point set, its graph, and the 3D line renderer it
/// draws through.
pub struct PlexusScene {
    lines: LineRenderer,
    nodes: InstancedQuads3d,
    /// The tier's point cap, the most points this scene will ever hold.
    points_cap: usize,
    /// The tier's edge cap.
    edges_cap: usize,
    /// The tier's cap on the circle of confusion, in pixels.
    max_coc: f32,
    /// This frame's per-frame clamp — the edge cap or the blur cap — if one
    /// bit, for the renderer to announce (ADR-0007: a cap is never silent).
    clamp: Option<super::CapOverflow>,
    points: sim::Points,
    edges: Vec<sim::Edge>,
    instances: Vec<Segment3dInstance>,
    node_instances: Vec<Quad3dInstance>,
    palette: Palette,
    dt: f32,
    /// The render target's size in pixels, handed in every frame.
    target: (u32, u32),

    link_distance: f32,
    link_alpha: f32,
    line_width: f32,
    node_size: f32,
    node_glow: f32,
    drift: f32,
    wave: f32,
    wave_scale: f32,
    hue_center: f32,
    hue_spread: f32,
    zoom: f32,
    colour: PaletteParams,
    pan: PanParams,
    camera: CameraParams,
}

impl PlexusScene {
    /// Build the scene with buffers for `points_cap` points and `edges_cap`
    /// edges, and blur held to `max_coc` pixels — the tier's caps.
    pub(crate) fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        points_cap: usize,
        edges_cap: usize,
        max_coc: f32,
    ) -> Self {
        let config = PlexusConfig::default();
        let count = (config.points as usize).min(points_cap);
        Self {
            lines: LineRenderer::new_3d(device, surface_format, edges_cap, "plexus"),
            nodes: InstancedQuads3d::new(device, "plexus", points_cap, surface_format),
            points_cap,
            edges_cap,
            max_coc,
            clamp: None,
            points: sim::Points::seeded(config.layout, config.seed, count, points_cap),
            edges: Vec::with_capacity(edges_cap),
            instances: Vec::with_capacity(edges_cap),
            node_instances: Vec::with_capacity(points_cap),
            palette: Palette::default_spectrum(),
            dt: super::FALLBACK_DT,
            target: (1, 1),
            link_distance: DEFAULT_LINK_DISTANCE,
            link_alpha: DEFAULT_LINK_ALPHA,
            line_width: DEFAULT_LINE_WIDTH,
            node_size: DEFAULT_NODE_SIZE,
            node_glow: DEFAULT_NODE_GLOW,
            drift: DEFAULT_DRIFT,
            wave: DEFAULT_WAVE,
            wave_scale: DEFAULT_WAVE_SCALE,
            hue_center: DEFAULT_HUE_CENTER,
            hue_spread: DEFAULT_HUE_SPREAD,
            zoom: DEFAULT_ZOOM,
            colour: PaletteParams::new(0.0, DEFAULT_BRIGHTNESS),
            pan: PanParams::default(),
            camera: CameraParams::default(),
        }
    }

    /// The palette coordinate at normalized depth `depth01`, `0` at the
    /// volume's nearest extent and `1` at its farthest (ADR-0059: a scene
    /// colours along its own generator's axis, and depth is this one's).
    fn colour_at(&self, depth01: f32) -> [f32; 3] {
        depth_colour(
            &self.palette,
            &self.colour,
            self.hue_center,
            self.hue_spread,
            depth01,
        )
    }
}

impl Scene for PlexusScene {
    fn name(&self) -> &'static str {
        "plexus"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        self.target = (width, height);
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<super::CapOverflow> {
        let GeneratorConfig::Plexus(config) = cfg else {
            return None;
        };
        // A switch starts the incoming preset from its own seed, with the
        // point count held to the tier and the clamp returned for the renderer
        // to announce with the preset.
        let (count, overflow) = points_clamp(config.points, self.points_cap);
        self.points = sim::Points::seeded(config.layout, config.seed, count, self.points_cap);
        self.edges.clear();
        // The outgoing preset's per-frame clamp is not this one's to report.
        self.clamp = None;
        overflow
    }

    fn mirror_overflow(&self) -> Option<&super::CapOverflow> {
        self.clamp.as_ref()
    }

    fn reset_params(&mut self) {
        self.link_distance = DEFAULT_LINK_DISTANCE;
        self.link_alpha = DEFAULT_LINK_ALPHA;
        self.line_width = DEFAULT_LINE_WIDTH;
        self.node_size = DEFAULT_NODE_SIZE;
        self.node_glow = DEFAULT_NODE_GLOW;
        self.drift = DEFAULT_DRIFT;
        self.wave = DEFAULT_WAVE;
        self.wave_scale = DEFAULT_WAVE_SCALE;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.zoom = DEFAULT_ZOOM;
        self.colour.reset();
        self.pan.reset();
        self.camera.reset();
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`, `render::camera`).
        if self.colour.set(name, value) || self.pan.set(name, value) || self.camera.set(name, value)
        {
            return;
        }
        match name {
            "link_distance" => self.link_distance = value,
            "link_alpha" => self.link_alpha = value,
            "line_width" => self.line_width = value,
            "node_size" => self.node_size = value,
            "node_glow" => self.node_glow = value,
            "drift" => self.drift = value,
            "wave" => self.wave = value,
            "wave_scale" => self.wave_scale = value,
            "hue_center" => self.hue_center = value,
            "hue_spread" => self.hue_spread = value,
            "zoom" => self.zoom = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        self.points
            .step(self.dt, self.drift, self.wave, self.wave_scale);
        let linked = sim::link(
            self.points.pos(),
            self.points.fade(),
            self.link_distance,
            self.edges_cap,
            &mut self.edges,
        );
        self.clamp = (linked > self.edges.len()).then(|| super::CapOverflow {
            dropped: linked - self.edges.len(),
            context: super::OverflowContext::Edges(linked as u32),
            cap: self.edges_cap,
        });
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The aspect is the render target's, handed in here (ADR-0037).
        let frame = self.camera.frame(
            aspect,
            self.zoom,
            [self.pan.x, self.pan.y],
            self.target,
            self.points.bounding_radius(),
            self.max_coc,
        );
        let (cam, margin) = (frame.view, frame.margin);
        let (near_extent, span) = (frame.near_extent, frame.span);
        // The blur clamp is announced unless the graph's own cap already is.
        if self.clamp.is_none() {
            self.clamp = frame.blur;
        }
        let link_alpha = self.link_alpha.clamp(0.0, 1.0);
        let width = if self.line_width.is_finite() {
            self.line_width.max(0.0)
        } else {
            DEFAULT_LINE_WIDTH
        };

        let mut instances = std::mem::take(&mut self.instances);
        instances.clear();
        for edge in &self.edges {
            let (Some(&pa), Some(&pb)) = (
                self.points.pos().get(edge.a as usize),
                self.points.pos().get(edge.b as usize),
            ) else {
                continue;
            };
            let alpha = edge.presence * link_alpha;
            if alpha <= 0.0 {
                continue;
            }
            let Some((a, b)) = cam.clip_near(pa, pb) else {
                continue;
            };
            if cam.outside(a, b, margin) {
                continue;
            }
            let mid = [
                0.5 * (pa[0] + pb[0]),
                0.5 * (pa[1] + pb[1]),
                0.5 * (pa[2] + pb[2]),
            ];
            let depth01 = ((cam.depth(mid) - near_extent) / span).clamp(0.0, 1.0);
            instances.push(Segment3dInstance {
                a,
                b,
                color: self.colour_at(depth01),
                width,
                alpha,
            });
        }

        let uniform = frame.uniform;
        self.lines
            .draw_3d(queue, encoder, view, &uniform, 1.0, SOFTNESS, &instances);
        self.instances = instances;

        let mut nodes = std::mem::take(&mut self.node_instances);
        node_instances(
            &mut nodes,
            self.points.pos(),
            self.points.fade(),
            &cam,
            margin,
            self.node_size,
            |depth| self.colour_at(((depth - near_extent) / span).clamp(0.0, 1.0)),
        );
        self.nodes.draw(
            queue,
            encoder,
            view,
            &uniform,
            if self.node_glow.is_finite() {
                self.node_glow.max(0.0)
            } else {
                DEFAULT_NODE_GLOW
            },
            &nodes,
        );
        self.node_instances = nodes;
    }
}

/// `asked` points held to the tier's `cap`, and the overflow to announce when
/// the cap bit.
pub(crate) fn points_clamp(asked: u32, cap: usize) -> (usize, Option<super::CapOverflow>) {
    let asked_n = asked as usize;
    if asked_n <= cap {
        return (asked_n, None);
    }
    (
        cap,
        Some(super::CapOverflow {
            dropped: asked_n - cap,
            context: super::OverflowContext::Points(asked),
            cap,
        }),
    )
}

/// The colour at normalized depth `depth01`, `0` at the volume's nearest extent
/// and `1` at its farthest (ADR-0059: a scene colours along its own generator's
/// axis, and depth is this one's). Edges and nodes both take it, so a node is
/// the colour of the lines that meet at it.
///
/// The palette coordinate is `hue_center + (depth01 - 0.5) * hue_spread`, then
/// banded by `palette_steps` and crossfaded by `palette_mix` through the shared
/// `palette::band_coord` and `Palette::sample` — the arithmetic every
/// CPU-coloured scene uses.
pub(crate) fn depth_colour(
    palette: &Palette,
    colour: &PaletteParams,
    hue_center: f32,
    hue_spread: f32,
    depth01: f32,
) -> [f32; 3] {
    let coord = hue_center + (depth01 - 0.5) * hue_spread;
    let rgb = palette::desaturate(
        palette.sample(palette::band_coord(coord, colour.steps), colour.mix),
        colour.saturation,
    );
    let b = colour.brightness;
    [rgb[0] * b, rgb[1] * b, rgb[2] * b]
}

/// A node at every visible point into `out` (cleared first): each point in
/// front of the near plane and not outside the frame by more than `margin`, of
/// radius `node_size` pixels at the focal plane and coloured by `colour` of its
/// view depth, dimmed by its face fade.
///
/// **`node_size <= 0` leaves `out` empty**, so a preset without dots issues no
/// sprites at all rather than drawing invisible ones.
pub(crate) fn node_instances(
    out: &mut Vec<Quad3dInstance>,
    pos: &[[f32; 3]],
    fade: &[f32],
    cam: &crate::render::camera::CameraView,
    margin: f32,
    node_size: f32,
    colour: impl Fn(f32) -> [f32; 3],
) {
    out.clear();
    if !node_size.is_finite() || node_size <= 0.0 {
        return;
    }
    for (p, f) in pos.iter().zip(fade) {
        if *f <= 0.0 {
            continue;
        }
        let depth = cam.depth(*p);
        if depth < crate::render::camera::NEAR || cam.outside(*p, *p, margin) {
            continue;
        }
        let [r, g, b] = colour(depth);
        out.push(Quad3dInstance {
            center: *p,
            radius: node_size,
            color: [r * f, g * f, b * f],
        });
    }
}

#[cfg(test)]
mod tests;
