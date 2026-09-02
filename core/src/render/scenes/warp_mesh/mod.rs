//! Warp mesh: a per-vertex UV grid that resamples the previous frame
//! (ADR-0113).
//!
//! # What it generalizes
//!
//! ADR-0048 gave the engine *one* affine transform through which an accumulation
//! reads its own past: a single zoom, rotation and translation applied
//! identically to every texel. This scene is that transform **per vertex**. The
//! frame is covered by a grid of cells; each of its vertices carries its own
//! `zoom`/`rot`/`cx`/`cy`/`dx`/`dy`/ `sx`/`sy`/`warp`, and the rasterizer
//! interpolates between them — so the past can spiral in one corner and drift in
//! another, which no single affine can express.
//!
//! Those nine outputs come from a preset's `[per_vertex]` table, whose bindings
//! are evaluated once per vertex per frame with `x`, `y`, `rad` and `ang` bound
//! to that vertex's own position. A preset that declares no such table gets the
//! scalar params of the same names applied everywhere, which is exactly ADR-0048's
//! single shared transform — so the idiom degrades to the one it generalizes.
//!
//! # The grid is a resolution, not a shape
//!
//! ADR-0037, and this is the most likely place in the engine to get it wrong,
//! because here the grid is *user-visible*: a preset names `[mesh] x` and `[mesh]
//! y`, and they are quantized and clamped to a tier capacity. **Every
//! screen-destined coordinate here takes its aspect from the render target** — the
//! `rad`/`ang` the per-vertex program reads (computed in `vertex_position`), and
//! the isotropic space the source-uv transform works in (computed in the vertex
//! shader from a uniform the CPU fills with the *target's* aspect).
//! `meshx`/`meshy` appear in neither. A `f32` aspect derived from the mesh size
//! would be the bug.
//!
//! # Three passes
//!
//! 1. **warp** — the mesh is drawn into the write half of a ping-pong field,
//!    sampling the read half through each vertex's source uv and scaling it by
//!    `decay^dt`. This is the only pass that is not fullscreen.
//! 2. **deposit** — a fullscreen pass adding this frame's light onto the warped
//!    past: a palette-coloured gaussian ring with optional angular arms. It runs
//!    *after* the warp, so the light it lays down is "now" and is warped from the
//!    next frame onward.
//! 3. **present** — a fullscreen pass compositing the field over the backdrop,
//!    premultiplied (ADR-0026), scaled by `brightness` and `occlude`.
//!
//! All three rates are **per second** (ADR-0019): `decay`, `zoom`, `sx`, `sy` are
//! factors per second and `rot`/`dx`/`dy`/`warp`/`deposit` are amounts per second,
//! so the look is identical at 60 Hz and 144 Hz.
//!
//! GPU resources are built lazily on first render, for the reason
//! `reaction_diffusion.rs` documents: a capture that never activates this scene
//! never builds this scene's pipelines, so it cannot perturb another scene's
//! render on the DX12 WARP software adapter.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::dsp::AnalysisFrame;
use crate::render::feedback::PingPongField;
use crate::render::gpu;
use crate::render::palette::{self, Palette};

use super::common;
use super::{Phase, Scene, lines};

// The five concerns of this scene, taking the shape `particles/` already has.
// `shaders` is the WGSL and the POD blocks,
// `mesh` the grid arithmetic and the CPU-side vertex assembly, `resources` the
// wgpu objects, `encode` the per-frame stages; what stays here is the scene, its
// `Scene` impl and the param surface.
mod encode;
mod mesh;
mod resources;
mod shaders;

// The grid bounds and the three grid functions were `pub` here before the split
// and are named from outside `warp_mesh` -- the renderer sizes its per-vertex
// scratch off `vertex_count` and evaluates bindings at `vertex_position`, and
// the preset schema validates a `[mesh]` table against the bounds -- so they
// keep their old path rather than gaining a `mesh::` segment.
pub use mesh::{DEFAULT_MESH, MAX_MESH, MIN_MESH, clamp_grid, vertex_count, vertex_position};

use mesh::*;
use resources::*;
use shaders::*;

/// The most vertices the filled-shape buffer holds.
///
/// Four shapes at MilkDrop's own limits — 1 024 instances of a 100-sided
/// polygon each — would be 1.2 M triangles, which is not a picture. This is the
/// bound that keeps the buffer a fixed allocation: past it the extra triangles
/// are dropped, which degrades a preset that asks for more rather than letting it
/// grow a buffer on the render thread.
pub const MAX_SHAPE_VERTICES: usize = 96 * 1024;

/// The nine outputs a `[per_vertex]` table may bind, in the order this scene
/// stores them. **Keep in step with `PER_VERTEX_DEFAULTS` and
/// `WarpMeshScene::set_per_vertex`.**
///
/// The same nine names are ordinary scalar [`PARAMS`] as well, and that is the
/// design: a scalar sets the output for the whole mesh, and a `[per_vertex]`
/// binding of the same name **replaces** it vertex by vertex. A preset therefore
/// starts from one shared transform and opts into a spatially-varying one output
/// at a time.
pub const PER_VERTEX_PARAMS: &[&str] = &["zoom", "rot", "cx", "cy", "dx", "dy", "sx", "sy", "warp"];

/// Each [`PER_VERTEX_PARAMS`] entry's identity value, positionally.
///
/// The identity of the whole roster is "the past sits still": unit scale, no
/// rotation, no drift, centred, no procedural warp — the same identity
/// [`Transform::IDENTITY`](crate::render::feedback::Transform::IDENTITY) names for
/// the affine this generalizes.
const PER_VERTEX_DEFAULTS: [f32; 9] = [1.0, 0.0, 0.5, 0.5, 0.0, 0.0, 1.0, 1.0, 0.0];

/// How many per-vertex outputs there are — typed off the roster so the arrays
/// below cannot drift from it.
const OUTPUTS: usize = PER_VERTEX_DEFAULTS.len();
const _: () = assert!(
    OUTPUTS == PER_VERTEX_PARAMS.len(),
    "the per-vertex roster and its defaults must be the same length"
);

/// `decay` default: 0.72 of the past survives each second, so a deposited streak
/// fades over roughly a second and a half.
const DEFAULT_DECAY: f32 = 0.72;
/// The most of the past a preset may keep per second. Not 1.0: at exactly 1 the
/// field is a perfect integrator and any deposit accumulates without bound, which
/// in linear light is a slowly whitening frame rather than a clip. Mirrors the
/// `MAX_FADE` ceiling the trails accumulation takes for the same reason.
const MAX_DECAY: f32 = 0.995;

/// The `softness` every `warp_mesh` stroke is drawn at — the waveform, every
/// custom wave, every shape outline, both borders and the motion grid, which all
/// reach the line fragment through one [`LineRenderer::draw_split`](lines::LineRenderer::draw_split)
/// call.
///
/// **Pinned at `1.0` — the pre-Plan-0114 profile — and it does NOT follow**
/// [`lines::DEFAULT_SOFTNESS`], which Plan 0114 Phase 5 moves (ADR-0124,
/// Alternative D0). The two constants exist because there are two judges: the
/// four line families answer to that plan's look gate, and this surface answers
/// to **`foo_vis_milk2`**, ADR-0113's fidelity reference, against which the
/// conversion has already been judged side by side. `draw.rs`'s stroke widths
/// were chosen *through* this profile — a thick MilkDrop line, drawn there as two
/// or four offset passes, reproduced here as one stroke of twice the width — so a
/// number picked by that gate answers a question nobody asked of this surface.
///
/// It is also the regime where the profile's `fwidth` term stops describing a
/// real gradient: `draw.rs`'s `THIN` is a **1.35 px** half-width at 1080p and
/// **1.0 px** at 1280x800. The pin stays byte-identical there only because the
/// edge term is capped at 1.0 — see the shared profile in the line renderer.
///
/// **Plan 0114 Phase 8 is what sets this**: it puts the reference rig beside a
/// spread of values and returns a number, and `1.0` — keeping the pin as it
/// stands — is a legitimate outcome that closes the question rather than a null
/// result. Until it runs, the pin holds the profile the conversion was judged
/// under.
pub const MILKDROP_SOFTNESS: f32 = 1.0;

/// Procedural-warp defaults — the spatial scale of the four sinusoids and how
/// fast they animate. `1.0` is MilkDrop's own unit scale.
const DEFAULT_WARP_SCALE: f32 = 1.0;
const DEFAULT_WARP_SPEED: f32 = 1.0;

/// Deposit defaults: a soft blob at the centre, bright enough to see and small
/// enough to be dragged into structure rather than filling the frame.
const DEFAULT_DEPOSIT: f32 = 1.6;
const DEFAULT_DEPOSIT_CENTRE: f32 = 0.5;
const DEFAULT_DEPOSIT_RADIUS: f32 = 0.45;
const DEFAULT_DEPOSIT_WIDTH: f32 = 0.11;
const DEFAULT_DEPOSIT_ARMS: f32 = 0.0;
const DEFAULT_DEPOSIT_TWIST: f32 = 0.0;
const DEFAULT_DEPOSIT_SPIN: f32 = 0.0;

/// **MilkDrop's composite roster**, in the order [`COMPOSITE_PARAMS`] declares
/// it — the six flags and one multiplier its format carries, reachable from a
/// preset and written by a converted bundle's per-frame program.
///
/// They are here rather than warned about because the corpus says so. Counted
/// over all 10 347 files, 2026-08-16:
///
/// ```text
/// bTexWrap=1       6 014   58 %
/// bDarken=1        3 686   36 %
/// bBrighten=1      1 445   14 %
/// bDarkenCenter=1    711    7 %
/// bInvert=1          576    6 %
/// bSolarize=1        445    4 %
/// ```
///
/// Each is one `select` in a shader, and between them they reach most of the
/// library.
///
/// **The video echo joined them in Plan 0109 Phase 3**, and it is the one member
/// that is not a remap: `echo_alpha`/`echo_zoom`/`echo_orient` blend a second
/// sampled copy of the finished field over the first. Only 252 files (2.4 %) set
/// a non-zero echo alpha, which is why it waited — but where it appears it is
/// load-bearing rather than decorative, and *Songflower (Moss Posy)*'s woven
/// lattice is only one family of bars without it.
pub const COMPOSITE_PARAMS: &[&str] = &[
    "gamma",
    "wrap",
    "darken_center",
    "brighten",
    "darken",
    "solarize",
    "invert",
    "echo_alpha",
    "echo_zoom",
    "echo_orient",
];

/// `gamma` default — MilkDrop's `fGammaAdj` at unity.
const DEFAULT_GAMMA: f32 = 1.0;
/// The other six default off, which is the identity for each.
const DEFAULT_COMPOSITE_FLAG: f32 = 0.0;
/// The echo's own defaults — no second copy, at unit zoom and unflipped, which
/// is the identity and is MilkDrop's own resting value for each.
const DEFAULT_ECHO_ALPHA: f32 = 0.0;
const DEFAULT_ECHO_ZOOM: f32 = 1.0;
const DEFAULT_ECHO_ORIENT: f32 = 0.0;

/// MilkDrop's `nVideoEchoOrientation` as its two flip bits — `1` flips x, `2`
/// flips y, `3` both.
///
/// **This is where a continuous value becomes one of four states.** The source
/// format stores an integer, but it reaches here as an `f32` that a per-frame
/// program can compute and that a preset's own smoothing can sweep *between*
/// states; deciding what `1.5` means in the shader would mean deciding it four
/// times. Out of range **wraps** rather than clamping, so a preset animating the
/// orientation by counting gets a cycle rather than a value stuck at `3`. Total
/// on every input, `NaN` included, because a non-finite orientation is not a
/// reason to lose the echo.
fn echo_orientation(v: f32) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    match v.round().rem_euclid(4.0) as i32 {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

/// How much `darken_center` takes out of the middle at full strength.
///
/// MilkDrop draws a fixed alpha there rather than exposing an amount; this is
/// that gesture as a multiplier, matched by eye to the reference's blob. A
/// preset binding a fraction gets a proportional one, which the format cannot
/// express and costs nothing to allow.
const DARKEN_CENTER_STRENGTH: f32 = 0.22;

/// Colour defaults (ADR-0021), matching the shared vocabulary every other
/// scene uses.
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_COLOR_SPAN: f32 = 1.0;
const DEFAULT_COLOR_CENTER: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    // The nine per-vertex outputs, as whole-mesh scalars.
    "zoom",
    "rot",
    "cx",
    "cy",
    "dx",
    "dy",
    "sx",
    "sy",
    "warp",
    // The procedural warp's own shape.
    "warp_scale",
    "warp_speed",
    // The feedback field.
    "decay",
    // This frame's light.
    "deposit",
    "deposit_x",
    "deposit_y",
    "deposit_radius",
    "deposit_width",
    "deposit_arms",
    "deposit_twist",
    "deposit_spin",
    // MilkDrop's composite roster (see `COMPOSITE_PARAMS`).
    "gamma",
    "wrap",
    "darken_center",
    "brighten",
    "darken",
    "solarize",
    "invert",
    "echo_alpha",
    "echo_zoom",
    "echo_orient",
    // Colour.
    "hue",
    "color_span",
    "color_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "brightness",
];

/// The warp mesh scene (ADR-0113).
pub struct WarpMeshScene {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    /// The tier's mesh ceiling, fixed for the life of the scene like every other
    /// tier capacity — a tier change rebuilds the scene.
    tier_mesh: (u32, u32),
    /// The grid the active preset asked for, before the tier clamp.
    requested_mesh: (u32, u32),
    state: MeshState,
    /// This frame's target size, recorded by `set_target_size` and acted on in
    /// `render` (ADR-0030: never allocate in the setter).
    target: (u32, u32),
    /// The **render target's** aspect as of the last `render` (ADR-0037).
    /// Recorded because a bundle's per-frame program reads `aspectx`/`aspecty`
    /// and `update` runs before `render` — see `update`.
    last_aspect: f32,
    time: f32,
    dt: f32,
    /// The nine per-vertex outputs as whole-mesh scalars, in
    /// [`PER_VERTEX_PARAMS`] order.
    scalars: [f32; OUTPUTS],
    warp_scale: f32,
    warp_speed: f32,
    /// The integrated warp phase ([`Phase`]): `+= warp_speed * dt` once per
    /// frame, in `update`, after this frame's parameter values have landed. At a
    /// constant rate it equals `warp_speed * time`, which is why the `1.0`
    /// default renders exactly as the multiply it replaced.
    warp_phase: Phase,
    decay: f32,
    deposit: f32,
    deposit_x: f32,
    deposit_y: f32,
    deposit_radius: f32,
    deposit_width: f32,
    deposit_arms: f32,
    deposit_twist: f32,
    deposit_spin: f32,
    /// The integrated deposit-arm rotation ([`Phase`]), beside `warp_phase` and
    /// for the same reason (ADR-0135): a rate multiplying the shared clock lets
    /// a binding that moves rescale all elapsed time in one frame.
    deposit_phase: Phase,
    gamma: f32,
    wrap: f32,
    darken_center: f32,
    brighten: f32,
    darken: f32,
    solarize: f32,
    invert: f32,
    echo_alpha: f32,
    echo_zoom: f32,
    echo_orient: f32,
    /// The shared palette knobs (ADR-0021). This scene has no `pan_*`.
    colour: common::PaletteParams,
    color_span: f32,
    color_center: f32,
    occlude: f32,
    /// The active baked palette. Held here rather than only in the resources'
    /// [`palette::LutPair`] because the resources are rebuilt on a resize and
    /// built lazily: this is what seeds a fresh pair.
    palette: Palette,
    /// The converted MilkDrop preset's live EEL2 state, when the preset carries a
    /// `[milk]` table (Plan 0100 Phase 2 / ADR-0113). `None` — a hand-authored
    /// preset — executes no VM at all, so the ten native systems and a native
    /// `warp_mesh` preset take exactly the path they took before this existed.
    ///
    /// **The bundle drives the scene *after* the ordinary bindings**, and that is
    /// the composition rule: `set_param` and `set_per_vertex` run during the
    /// renderer's `evaluate_preset`, and the programs run in
    /// [`update`](Scene::update) and [`render`](Scene::render), which come later.
    /// A converted preset is authoritative about its own transform; a `[params]`
    /// binding alongside one is inert rather than fighting it.
    milk: Option<crate::milk::MilkRuntime>,
    /// What the bundle's translated shaders ask the scene to build (Plan 0100
    /// Phase 6). Extracted at `configure`; `render` compares its key against
    /// the built resources and rebuilds when a preset switch changes it.
    shader_spec: Option<shader::ShaderSpec>,
    /// How many levels the feedback field quantizes to at the end of the warp
    /// pass (ADR-0118), extracted from the bundle at `configure`. **`0.0` — no
    /// bundle — is off, and off is an exact identity**, so a native `warp_mesh`
    /// preset renders exactly what it rendered before this existed. Negative is
    /// the ADR's Alternative D. Both warp fragments read it: the converted one
    /// through `MilkUniform.misc.w`, the built-in one through
    /// `WarpUniform.misc3.x`.
    quantize_steps: f32,
    /// The tier's line-segment cap, which the draw layer's own `LineRenderer` is
    /// sized to — the same capacity every line scene gets (ADR-0045).
    max_segments: usize,
    /// This frame's draw-layer outputs, from the bundle's per-frame program.
    /// `None` for a hand-authored preset, which draws no MilkDrop layer.
    draw: Option<crate::milk::outputs::FrameOutputs>,
    /// The CPU-side geometry the draw layer builds each frame. Its capacity is
    /// reused, so the per-frame path allocates nothing after the first frames.
    geometry: draw::DrawGeometry,
    /// This frame's analysis, kept from `update` so `render` can drive the
    /// per-vertex program with the same frame the per-frame program saw.
    frame: crate::dsp::AnalysisFrame,
}

impl WarpMeshScene {
    /// The feedback field's readable texture, or `None` before the first render
    /// has built the GPU resources.
    ///
    /// **Test-only, and it is the seam-A tap** (Plan 0111 Phase 2): the field is
    /// what the present pass reads and everything downstream is what the bisect
    /// covers, so a probe needs the value *before* the present pass to have a
    /// baseline at all. `PingPongField` already carries `COPY_SRC` for Plan 0109
    /// Phase 4's probe; this only names it from outside the module, which is what
    /// lets a `Renderer`-level probe read the same quantity the scene-level one
    /// does rather than approximating it.
    #[cfg(test)]
    pub(crate) fn field_texture(&self) -> Option<&wgpu::Texture> {
        Some(self.res.as_ref()?.field.read_texture())
    }

    /// Build the CPU-side state. GPU resources are deferred to the first render
    /// (module docs). `tier_mesh` is the active tier's
    /// [`mesh_grid`](crate::render::TierConfig::mesh_grid).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        tier_mesh: (u32, u32),
        max_segments: usize,
    ) -> Self {
        let tier = crate::render::TierConfig {
            mesh_grid: tier_mesh,
            ..crate::render::TierConfig::FLOOR
        };
        let mesh = clamp_grid(DEFAULT_MESH, &tier);
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            tier_mesh,
            requested_mesh: DEFAULT_MESH,
            state: MeshState::new(mesh),
            target: (0, 0),
            last_aspect: 1.0,
            time: 0.0,
            dt: super::FALLBACK_DT,
            scalars: PER_VERTEX_DEFAULTS,
            warp_scale: DEFAULT_WARP_SCALE,
            warp_speed: DEFAULT_WARP_SPEED,
            warp_phase: Phase::default(),
            decay: DEFAULT_DECAY,
            deposit: DEFAULT_DEPOSIT,
            deposit_x: DEFAULT_DEPOSIT_CENTRE,
            deposit_y: DEFAULT_DEPOSIT_CENTRE,
            deposit_radius: DEFAULT_DEPOSIT_RADIUS,
            deposit_width: DEFAULT_DEPOSIT_WIDTH,
            deposit_arms: DEFAULT_DEPOSIT_ARMS,
            deposit_twist: DEFAULT_DEPOSIT_TWIST,
            deposit_spin: DEFAULT_DEPOSIT_SPIN,
            deposit_phase: Phase::default(),
            gamma: DEFAULT_GAMMA,
            wrap: DEFAULT_COMPOSITE_FLAG,
            darken_center: DEFAULT_COMPOSITE_FLAG,
            brighten: DEFAULT_COMPOSITE_FLAG,
            darken: DEFAULT_COMPOSITE_FLAG,
            solarize: DEFAULT_COMPOSITE_FLAG,
            invert: DEFAULT_COMPOSITE_FLAG,
            echo_alpha: DEFAULT_ECHO_ALPHA,
            echo_zoom: DEFAULT_ECHO_ZOOM,
            echo_orient: DEFAULT_ECHO_ORIENT,
            colour: common::PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
            palette: Palette::default_spectrum(),
            milk: None,
            shader_spec: None,
            quantize_steps: 0.0,
            max_segments,
            draw: None,
            geometry: draw::DrawGeometry::default(),
            frame: crate::dsp::AnalysisFrame::default(),
        }
    }

    /// The grid this scene actually draws, after the tier clamp. The renderer
    /// calls the same [`clamp_grid`] on the same request, so the per-vertex
    /// series it sends is exactly this long.
    pub fn mesh(&self) -> (u32, u32) {
        self.state.mesh
    }
}

impl Scene for WarpMeshScene {
    fn name(&self) -> &'static str {
        "warp mesh"
    }

    #[cfg(test)]
    fn feedback_field(&self) -> Option<&wgpu::Texture> {
        self.field_texture()
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn advance(&mut self, dt: f32) {
        // Every rate in this scene is per second, so the frame's own elapsed time
        // is the whole of what `advance` carries. A non-finite or negative `dt`
        // degrades to the capture step rather than poisoning `pow`.
        self.dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            super::FALLBACK_DT
        };
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        // Record only — ADR-0030 condition 2. `render` notices the difference.
        self.target = (width.max(1), height.max(1));
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        if let Some(res) = self.res.as_mut() {
            res.luts.set(palette);
        }
    }

    fn configure(&mut self, cfg: &super::GeneratorConfig) -> Option<super::CapOverflow> {
        if let super::GeneratorConfig::WarpMesh { mesh, milk, salt } = cfg {
            self.requested_mesh = *mesh;
            let tier = crate::render::TierConfig {
                mesh_grid: self.tier_mesh,
                ..crate::render::TierConfig::FLOOR
            };
            self.state.resize(clamp_grid(*mesh, &tier));
            // Built here, off the hot path, and rebuilt on every preset switch —
            // so a bundle never inherits the previous preset's register file,
            // `megabuf` or RNG stream. `configure` runs on every switch for
            // exactly this reason (the `[particles]` arm's note).
            self.milk = milk
                .as_ref()
                .map(|bundle| crate::milk::MilkRuntime::new((**bundle).clone(), *salt));
            // The feedback quantizer's step count (ADR-0118). **A bundle decides
            // it; the absence of one is the decision for a native preset**, and
            // that split is the whole per-bundle shape — `warp_mesh` is a native
            // scene too, and a hand-authored world has no reason to want an
            // 8-bit-era feedback field.
            self.quantize_steps = milk.as_ref().map_or(0.0, |bundle| bundle.quantize_steps);
            // The translated shaders, when the bundle carries any (Phase 6).
            self.shader_spec = milk.as_ref().and_then(|bundle| {
                (bundle.warp_wgsl.is_some() || bundle.comp_wgsl.is_some()).then(|| {
                    shader::ShaderSpec {
                        warp: bundle.warp_wgsl.clone(),
                        comp: bundle.comp_wgsl.clone(),
                        blur: bundle.blur_level,
                    }
                })
            });
            // A preset switch must not leave the previous bundle's draw layer
            // on screen for a frame.
            self.draw = None;
            self.geometry.clear();
        }
        None
    }

    fn reset_params(&mut self) {
        self.scalars = PER_VERTEX_DEFAULTS;
        // A `[per_vertex]` binding is re-applied every frame, so clearing the
        // flags here is what makes an unbound output fall back to its scalar.
        self.state.bound = [false; OUTPUTS];
        self.warp_scale = DEFAULT_WARP_SCALE;
        self.warp_speed = DEFAULT_WARP_SPEED;
        self.decay = DEFAULT_DECAY;
        self.deposit = DEFAULT_DEPOSIT;
        self.deposit_x = DEFAULT_DEPOSIT_CENTRE;
        self.deposit_y = DEFAULT_DEPOSIT_CENTRE;
        self.deposit_radius = DEFAULT_DEPOSIT_RADIUS;
        self.deposit_width = DEFAULT_DEPOSIT_WIDTH;
        self.deposit_arms = DEFAULT_DEPOSIT_ARMS;
        self.deposit_twist = DEFAULT_DEPOSIT_TWIST;
        self.deposit_spin = DEFAULT_DEPOSIT_SPIN;
        self.gamma = DEFAULT_GAMMA;
        self.wrap = DEFAULT_COMPOSITE_FLAG;
        self.darken_center = DEFAULT_COMPOSITE_FLAG;
        self.brighten = DEFAULT_COMPOSITE_FLAG;
        self.darken = DEFAULT_COMPOSITE_FLAG;
        self.solarize = DEFAULT_COMPOSITE_FLAG;
        self.invert = DEFAULT_COMPOSITE_FLAG;
        self.echo_alpha = DEFAULT_ECHO_ALPHA;
        self.echo_zoom = DEFAULT_ECHO_ZOOM;
        self.echo_orient = DEFAULT_ECHO_ORIENT;
        self.colour.reset();
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) {
            return;
        }
        // The nine per-vertex outputs, as whole-mesh scalars — the fallback a
        // `[per_vertex]` binding of the same name overrides.
        if let Some(index) = PER_VERTEX_PARAMS.iter().position(|n| *n == name) {
            if let Some(slot) = self.scalars.get_mut(index) {
                *slot = value;
            }
            return;
        }
        match name {
            "warp_scale" => self.warp_scale = value,
            "warp_speed" => self.warp_speed = value,
            "decay" => self.decay = value,
            "deposit" => self.deposit = value,
            "deposit_x" => self.deposit_x = value,
            "deposit_y" => self.deposit_y = value,
            "deposit_radius" => self.deposit_radius = value,
            "deposit_width" => self.deposit_width = value,
            "deposit_arms" => self.deposit_arms = value,
            "deposit_twist" => self.deposit_twist = value,
            "deposit_spin" => self.deposit_spin = value,
            "gamma" => self.gamma = value,
            "wrap" => self.wrap = value,
            "darken_center" => self.darken_center = value,
            "brighten" => self.brighten = value,
            "darken" => self.darken = value,
            "solarize" => self.solarize = value,
            "invert" => self.invert = value,
            "echo_alpha" => self.echo_alpha = value,
            "echo_zoom" => self.echo_zoom = value,
            "echo_orient" => self.echo_orient = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            _ => {}
        }
    }

    fn set_per_vertex(&mut self, name: &str, values: &[f32]) {
        let Some(index) = PER_VERTEX_PARAMS.iter().position(|n| *n == name) else {
            return;
        };
        let Some(slot) = self.state.values.get_mut(index) else {
            return;
        };
        // A series of the wrong length means the renderer and this scene clamped
        // the grid differently, which `clamp_grid` exists to prevent. Copy what
        // fits and leave the rest at the scalar rather than panicking on the hot
        // path.
        let n = slot.len().min(values.len());
        if let (Some(dst), Some(src)) = (slot.get_mut(..n), values.get(..n)) {
            dst.copy_from_slice(src);
        }
        if let Some(flag) = self.state.bound.get_mut(index) {
            *flag = n > 0;
        }
    }

    fn update(&mut self, frame: &AnalysisFrame) {
        // The warp phase integrates here rather than in `advance`, because
        // `advance` runs before this frame's `set_param` calls and would
        // therefore use the previous frame's rate (ADR-0132).
        self.warp_phase.step(self.warp_speed, self.dt);
        self.deposit_phase.step(self.deposit_spin, self.dt);
        // Kept for `render`, which drives the per-vertex program and is the only
        // place the render target's aspect is known.
        self.frame = *frame;
        // A converted preset's per-frame program, run after the ordinary
        // bindings and overriding them — see the `milk` field.
        //
        // The aspect is deliberately **not** available here, so the value handed
        // to the program is the one `render` recorded last frame (or 1.0 on the
        // first). `aspectx`/`aspecty` change only on a resize, so a one-frame lag
        // on a window drag is invisible; taking the aspect from the mesh instead
        // would be the ADR-0037 bug.
        let aspect = self.last_aspect;
        let mesh = self.state.mesh;
        let (time, dt) = (self.time, self.dt);
        if let Some(runtime) = self.milk.as_mut() {
            let (transform, out) = runtime.run_frame(&self.frame, time, dt, mesh, aspect);
            for (index, value) in transform.iter().enumerate() {
                if let Some(slot) = self.scalars.get_mut(index) {
                    *slot = *value;
                }
            }
            // The composite roster, by field rather than by a positional table —
            // the whole point of `outputs::FrameOutputs` (Plan 0100 Phase 4).
            self.decay = out.decay;
            self.gamma = out.gamma;
            self.wrap = out.wrap;
            self.darken_center = out.darken_center;
            self.brighten = out.brighten;
            self.darken = out.darken;
            self.solarize = out.solarize;
            self.invert = out.invert;
            self.echo_alpha = out.echo_alpha;
            self.echo_zoom = out.echo_zoom;
            self.echo_orient = out.echo_orient;
            // **The deposit is NOT forced off here**, and that was a bug for one
            // commit. A converted preset draws its own light — the waveform, its
            // custom elements, its borders — and the converter emits no deposit
            // bindings for exactly that reason, so it already gets none. Forcing
            // it off in the scene instead would also silence a HAND-WRITTEN
            // bundle that uses the deposit as its light source, which is a
            // perfectly good thing for one to do and is what
            // `core/tests/fixtures/warp_mesh_milk.toml` does.
            self.draw = Some(out);
            // A bundle's per-vertex program replaces any `[per_vertex]` table's
            // series wholesale, so the flags are cleared here and re-set in
            // `render` once the vertices are evaluated.
            self.state.bound = [false; OUTPUTS];
        }
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let size = if self.target == (0, 0) {
            // No `set_target_size` yet (a caller that renders without the
            // renderer's per-frame hook). Fall back to a square field rather
            // than allocating nothing.
            (256, 256)
        } else {
            self.target
        };
        if !encode::ensure_resources(self, queue, encoder, size) {
            return;
        }

        self.last_aspect = aspect;
        encode::prepare_mesh(self, aspect);

        let dt = self.dt;
        if let Some(res) = self.res.as_ref() {
            encode::upload_uniforms(self, res, queue, aspect, size, dt);
            encode::encode_warp(res, encoder);
            encode::encode_deposit(res, encoder);
        }
        encode::encode_draw_layer(self, queue, encoder, aspect, dt);
        if let Some(res) = self.res.as_ref() {
            encode::encode_blur(res, encoder);
            encode::encode_present(res, encoder, view);
        }
    }
}

pub mod draw;
mod shader;

#[cfg(test)]
mod tests;
