//! Particle-swarm scene: ~10k CPU-simulated particles drifting through a flow
//! field, drawn as instanced additive sprites (the starfield's approach,
//! scaled up). One of the two preset-driven systems (ADR-0002 layers 1-2).
//!
//! Its behavior is a set of named parameters — `force`, `spin`, `burst`, `hue`,
//! `brightness`, `size` — that a preset binds to expressions over the audio
//! analysis (Plan 0003 Phase 5). All per-particle math is CPU-side; no compute
//! shader. Motion is deterministic; the only randomness is the seeded initial
//! scatter (NFR 6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::{FALLBACK_DT, Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

/// Particle count. 10k is the target look (Plan 0003); it holds the primary
/// dev box comfortably and is the number to validate against the 60 fps @
/// 1080p floor on the iGPU test PC (NFR 1/9), reducing here if it misses.
const PARTICLES: usize = 10_000;
const SEED: u64 = 0x4C4D_565F_5357_524D; // "LMV_SWRM"

/// How far the toroidal domain extends past the visible frame (Plan 0043 Phase 1,
/// [ADR-0044](../../../../docs/adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md)).
///
/// The world half-extents used to be `BOUND_X = 1.8` / `BOUND_Y = 1.0` — and
/// `1.0` **is** the NDC frame edge. The wrap is toroidal, so that line was the one
/// place on screen every wrapping particle was guaranteed to paint, and the
/// feedback stage integrated it into a saturated bar across the top and bottom of
/// every swarm preset within a few hundred frames.
///
/// The bounds now follow the render target (below) and carry this margin so the
/// seam sits *outside* the frame. Chosen by measurement, not by rounding: the
/// family works at `zoom` 1.0–1.3 with `pan_*` to about 0.16, and a particle at
/// world `y = BOUND_Y` lands on the frame edge when `BOUND_Y * zoom - |pan_y| ==
/// 1`. At the worst case in that range (`zoom = 1.0`) the seam clears the frame
/// for `|pan| <= MARGIN - 1`, so 1.25 buys 0.25 of pan headroom on both axes —
/// comfortably past what the family uses, and it also puts the *domain rectangle*
/// off-screen down to `zoom = 0.8`, which is the inset-edge wall that pinned the
/// family at or above 1.0.
///
/// The cost is visible density: the visible fraction of the domain is `1 /
/// MARGIN^2`, so a quarter of the 10 000 particles are off-screen at any moment.
/// That is the tradeoff Phase 4's re-authoring absorbs.
const MARGIN: f32 = 1.25;
/// Domain aspect before the first [`Scene::render`] hands one over. Only reached
/// on the very first `update` of a fresh scene; because positions are stored
/// normalized (see [`Particle::pos`]) an aspect change rescales the field rather
/// than teleporting it, so this fallback is continuous with whatever follows.
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Velocity retained per frame (the rest is re-steered by the flow field).
const DAMPING: f32 = 0.86;
/// Spatial frequency of the flow field.
const FIELD_FREQ: f32 = 2.3;

/// Parameter defaults — a calm idle drift when nothing is bound.
const DEFAULT_FORCE: f32 = 1.4;
const DEFAULT_SPIN: f32 = 0.3;
const DEFAULT_BURST: f32 = 0.0;
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 0.8;
const DEFAULT_SIZE: f32 = 1.0;
// Shared palette color knobs (ADR-0021). Each particle's hue occupies the band
// `hue_center + (particle_hue - 0.5) * hue_spread`; the defaults (`center = 0.5`,
// `spread = 1`) reproduce the prior full-wheel look (`particle_hue`), and
// `saturation = 1` leaves color untouched — so an unbound swarm is unchanged.
const DEFAULT_HUE_SPREAD: f32 = 1.0;
const DEFAULT_HUE_CENTER: f32 = 0.5;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only (the crossfade is a no-op unless a
/// preset declares `[palette_b]` and binds `palette_mix`).
const DEFAULT_PALETTE_MIX: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default, so an unbound preset is
// unchanged. `zoom` multiplies particle positions about the frame centre; `pan_*`
// offset them — matching the line scenes' semantics (zoom > 1 = zoomed in).
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;

const SHADER: &str = r#"
struct Misc {
    // x: aspect, y: zoom, zw: pan (the shared ViewTransform, ADR-0018)
    v: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: f32,
    @location(2) color: vec3<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    // Shared ViewTransform (ADR-0018): zoom about the frame centre, then pan the
    // particle position; the sprite quad (c * size) keeps its on-screen size.
    let zoom = misc.v.y;
    let pan = misc.v.zw;
    let center_v = center * zoom + pan;
    let world = center_v + c * size;
    var out: VsOut;
    out.pos = vec4<f32>(world.x / misc.v.x, world.y, 0.0, 1.0);
    out.local = c;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = length(in.local);
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    return vec4<f32>(in.color * g, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 2],
    size: f32,
    color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Misc {
    v: [f32; 4],
}

struct Particle {
    /// Position on the torus in **normalized** domain coordinates, each axis in
    /// `[-1, 1)`; world position is this times the current half-extents (Plan 0043
    /// Phase 1).
    ///
    /// Normalized rather than world-space for one reason: the half-extents now
    /// follow the render target, so they change on a resize, and a world-space
    /// store would have to either re-wrap (teleporting every particle that fell
    /// outside the new domain, all in one frame) or rescale every position by
    /// hand. Here the resize *is* the rescale — each particle keeps its place on
    /// the torus and the field stretches with the frame, which is the
    /// discontinuity-free resize ADR-0044 requires. It also keeps the seeded
    /// scatter aspect-independent, so the same seed gives the same field at any
    /// target size (NFR §6).
    pos: [f32; 2],
    /// Velocity in **world** units per second — the flow field and the burst are
    /// screen-space forces, so they must not change magnitude with the domain.
    vel: [f32; 2],
    /// Per-particle palette offset and brightness, from the seeded scatter.
    hue: f32,
    bright: f32,
    size: f32,
}

/// ~10k-particle CPU flow-field swarm, driven by named preset parameters.
pub struct SwarmScene {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    particles: Vec<Particle>,
    instance_data: Vec<Instance>,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    /// The **render target's** aspect, recorded by `render` for the next `update`
    /// to size the toroidal domain from (Plan 0043 Phase 1).
    ///
    /// Read off `render`'s argument and deliberately **not** off
    /// [`Scene::set_target_size`](super::Scene::set_target_size), which carries the
    /// post chain's internal grid — a quantized *resolution*, not a shape, whose
    /// aspect is only approximately the target's (ADR-0037). Every swarm preset
    /// composes `trails`, so that grid is exactly the quantized case; taking a
    /// domain shape from it is the defect ADR-0037 was written for.
    ///
    /// One frame behind by construction: `update` runs before `render` in a frame,
    /// so the domain follows the target with a single frame of lag. Harmless —
    /// positions are normalized, so a change rescales the field continuously.
    aspect: f32,
    /// Real elapsed seconds for this frame's integration (Plan 0014 Phase 2),
    /// injected via `advance` so the swarm moves at the same wall-clock rate on
    /// any refresh. Seeded to the fallback step for the first frame before any
    /// `advance` call.
    dt: f32,
    force: f32,
    spin: f32,
    burst: f32,
    hue: f32,
    brightness: f32,
    size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    /// The active baked palette (ADR-0021), sampled per particle on the CPU. Set
    /// by `set_palette` on a preset switch; default `spectrum` reproduces the
    /// prior cosine.
    palette: Palette,
    /// Per-particle hue band + shared desaturation (ADR-0021).
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    /// A/B palette crossfade position (Plan 0020 Phase 4); 0 = palette A.
    palette_mix: f32,
}

impl SwarmScene {
    /// Build the pipeline, buffers, and seeded particle set on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("swarm-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swarm-instances"),
            size: (PARTICLES * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swarm-misc"),
            size: std::mem::size_of::<Misc>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("swarm-bind-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("swarm-bind-group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("swarm-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("swarm-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32,
                        2 => Float32x3,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // Additive: overlapping particles bloom brighter.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let mut rng = SeededRng::new(SEED);
        let particles = (0..PARTICLES).map(|_| Self::spawn(&mut rng)).collect();

        Self {
            pipeline,
            instances,
            uniforms,
            bind_group,
            particles,
            instance_data: vec![
                Instance {
                    center: [0.0, 0.0],
                    size: 0.0,
                    color: [0.0, 0.0, 0.0],
                };
                PARTICLES
            ],
            time: 0.0,
            aspect: FALLBACK_ASPECT,
            dt: FALLBACK_DT,
            force: DEFAULT_FORCE,
            spin: DEFAULT_SPIN,
            burst: DEFAULT_BURST,
            hue: DEFAULT_HUE,
            brightness: DEFAULT_BRIGHTNESS,
            size: DEFAULT_SIZE,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            palette: Palette::default_spectrum(),
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
        }
    }

    /// A particle scattered across the field with a random heading and tint.
    ///
    /// The scatter is in **normalized** domain coordinates, so it does not depend
    /// on the render target — the same seed gives the same field at any size
    /// (NFR §6).
    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel index a fixed [f32; 2] at constant 0/1, always in-bounds"
    )]
    fn spawn(rng: &mut SeededRng) -> Particle {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        Particle {
            pos: [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)],
            vel: [angle.cos() * 0.2, angle.sin() * 0.2],
            hue: rng.next_f32(),
            bright: rng.range(0.5, 1.0),
            size: rng.range(0.004, 0.011),
        }
    }
}

/// The toroidal world half-extents for a render target of this aspect (Plan 0043
/// Phase 1).
///
/// The visible frame is `|world.y| <= 1` and `|world.x| <= aspect` — the shader
/// divides x by the aspect on its way to NDC — so this is the visible rectangle
/// scaled by [`MARGIN`], which is what puts the wrap seam off-screen. At
/// `MARGIN = 1` and 16:9 it returns `(1.78, 1.0)`, i.e. the constants it replaces.
fn bounds(aspect: f32) -> (f32, f32) {
    (aspect * MARGIN, MARGIN)
}

/// The LUT sample coordinate for one particle (ADR-0021): its per-particle hue
/// occupies the band `hue_center + (particle_hue - 0.5) * hue_spread`, plus the
/// shared `hue` rotation. Defaults (`center = 0.5`, `spread = 1`, `hue = 0`)
/// reduce to `particle_hue`, reproducing the prior full-wheel look.
fn hue_coord(hue_center: f32, hue_spread: f32, particle_hue: f32, hue: f32) -> f32 {
    hue_center + (particle_hue - 0.5) * hue_spread + hue
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "force",
    "spin",
    "burst",
    "hue",
    "brightness",
    "size",
    "zoom",
    "pan_x",
    "pan_y",
    "hue_spread",
    "hue_center",
    "saturation",
    "palette_mix",
];

impl Scene for SwarmScene {
    fn name(&self) -> &'static str {
        "swarm"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn set_palette(&mut self, palette: &Palette) {
        // CPU-sampled per particle in `update`; a cheap array copy, off the hot
        // path (once per preset switch).
        self.palette = palette.clone();
    }

    fn reset_params(&mut self) {
        self.force = DEFAULT_FORCE;
        self.spin = DEFAULT_SPIN;
        self.burst = DEFAULT_BURST;
        self.hue = DEFAULT_HUE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.size = DEFAULT_SIZE;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "force" => self.force = value,
            "spin" => self.spin = value,
            "burst" => self.burst = value,
            "hue" => self.hue = value,
            "brightness" => self.brightness = value,
            "size" => self.size = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            _ => {}
        }
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel index fixed [f32; 2] and base indexes a fixed [f32; 3], all at constant offsets, always in-bounds"
    )]
    fn update(&mut self, _frame: &AnalysisFrame) {
        // Field evolves at `spin`; `force` steers, `burst` shoves outward.
        let field_t = self.time * self.spin;
        let force = self.force;
        let burst_kick = self.burst;

        // Frame-rate-independent integration (Plan 0014 Phase 2): scale the
        // acceleration/advection by real `dt`, and raise the per-frame damping to
        // the `dt`-relative power so the velocity decays at the same wall-clock
        // rate regardless of refresh (one `powf` per frame, not per particle).
        // At `dt == FALLBACK_DT` (1/60) this reduces to the former fixed step, so
        // the look is unchanged live and byte-identical under fixed-`dt` capture.
        let dt = self.dt;
        let damp = DAMPING.powf(dt * 60.0);

        // The domain follows the render target (Plan 0043 Phase 1). Computed once
        // per frame, outside the loop; positions are normalized, so a change in
        // these rescales the whole field at once instead of wrapping particles
        // individually — no resize teleport (ADR-0044).
        let (bound_x, bound_y) = bounds(self.aspect);

        for (p, inst) in self.particles.iter_mut().zip(self.instance_data.iter_mut()) {
            // Normalized torus position -> world, which is what the field, the
            // burst and the sprite all work in.
            let world = [p.pos[0] * bound_x, p.pos[1] * bound_y];

            // Scalar potential -> flow direction (cheap curl-ish field).
            let a = (world[0] * FIELD_FREQ + field_t).sin()
                + (world[1] * FIELD_FREQ - field_t * 0.8).cos();
            let dir = [a.cos(), a.sin()];

            p.vel[0] = p.vel[0] * damp + dir[0] * force * dt;
            p.vel[1] = p.vel[1] * damp + dir[1] * force * dt;

            // Beat burst pushes particles radially outward from center.
            if burst_kick > 0.0 {
                let r = (world[0] * world[0] + world[1] * world[1]).sqrt().max(1e-3);
                p.vel[0] += world[0] / r * burst_kick * dt;
                p.vel[1] += world[1] / r * burst_kick * dt;
            }

            // Integrate a world-space velocity into a normalized position.
            p.pos[0] += p.vel[0] * dt / bound_x;
            p.pos[1] += p.vel[1] * dt / bound_y;

            // Toroidal wrap keeps the field populated (no respawns/hitches). In
            // normalized space the seam is at +/-1 whatever the target is, and it
            // is `MARGIN` past the visible frame — which is what stopped it from
            // burning a bright bar into the feedback stage (ADR-0044).
            if p.pos[0] > 1.0 {
                p.pos[0] -= 2.0;
            } else if p.pos[0] < -1.0 {
                p.pos[0] += 2.0;
            }
            if p.pos[1] > 1.0 {
                p.pos[1] -= 2.0;
            } else if p.pos[1] < -1.0 {
                p.pos[1] += 2.0;
            }

            let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
            // Colour through the shared LUT (ADR-0021): the per-particle hue is
            // mapped into the `hue_spread`/`hue_center` band, then desaturated by
            // the shared `saturation`. Defaults reproduce the prior full-wheel look.
            let coord = hue_coord(self.hue_center, self.hue_spread, p.hue, self.hue);
            let base = palette::desaturate(
                self.palette.sample(coord, self.palette_mix),
                self.saturation,
            );
            let bright = ((0.25 + speed * 0.7) * p.bright).min(1.6) * self.brightness;

            *inst = Instance {
                center: [p.pos[0] * bound_x, p.pos[1] * bound_y],
                size: p.size * self.size,
                color: [base[0] * bright, base[1] * bright, base[2] * bright],
            };
        }
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The domain the *next* `update` wraps against (Plan 0043 Phase 1). This
        // argument is the render target's aspect — the only correct source for a
        // shape (ADR-0037); see the field's docs for why `set_target_size` is not.
        self.aspect = aspect.max(0.1);
        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::cast_slice(&self.instance_data),
        );
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Misc {
                v: [self.aspect, self.zoom, self.pan_x, self.pan_y],
            }),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("swarm-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018): the additive
                    // particles bloom over whatever the background pass painted,
                    // so the sparse gaps between them reveal it.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..PARTICLES as u32);
    }
}

#[cfg(test)]
mod tests {
    // Test asserts index fixed-size arrays; allowed over the file's hot-path
    // pragma — this is not the render path.
    #![allow(clippy::indexing_slicing)]

    use super::{DEFAULT_HUE, DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, MARGIN, bounds, hue_coord};
    use crate::render::palette::Palette;

    /// Target aspects worth checking a domain against: 16:9, the 16:10 the fixed
    /// constants disagreed with, 4:3, an ultrawide, and a portrait.
    const ASPECTS: [f32; 5] = [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0];

    /// The domain has the **render target's** shape, not the baked 16:9 the
    /// replaced constants encoded (ADR-0037, ADR-0044).
    ///
    /// The visible frame is `|world.y| <= 1` by `|world.x| <= aspect`, so a domain
    /// that fills it without over-filling has exactly that ratio. The old
    /// `BOUND_X = 1.8` / `BOUND_Y = 1.0` pair is 1.80 at every target size — right
    /// only at 16:9, and the reason no existing test could tell: at 16:10 it
    /// over-fills horizontally by 12 %.
    #[test]
    fn the_domain_takes_its_shape_from_the_target() {
        for aspect in ASPECTS {
            let (bx, by) = bounds(aspect);
            assert!(
                (bx / by - aspect).abs() < 1e-5,
                "domain shape {:.4} must equal the target's {aspect:.4}",
                bx / by
            );
        }

        // The pair it replaced, for the record: correct at 16:9 and wrong the
        // moment the target is anything else.
        let (old_x, old_y) = (1.8f32, 1.0f32);
        let sixteen_ten = 16.0 / 10.0;
        assert!(
            (old_x / old_y - sixteen_ten).abs() > 0.1,
            "the fixed constants must genuinely disagree with 16:10, or this guards nothing"
        );
    }

    /// **The artifact fix, as arithmetic** (backlog 0029): the toroidal wrap seam
    /// projects outside the visible frame across the whole `zoom`/`pan_*` range the
    /// swarm family works in, so no particle is guaranteed to paint on a fixed
    /// on-screen line and the feedback stage has no bar to integrate.
    ///
    /// The shader projects a particle at world `p` to
    /// `ndc = ((p.x * zoom + pan_x) / aspect, p.y * zoom + pan_y)`, so a seam at
    /// `+bound` clears the frame when `bound * zoom - |pan| > extent`. Non-vacuous
    /// by construction: at the old `MARGIN = 1` equivalent the y seam lands exactly
    /// on `ndc.y = 1` at `zoom = 1`, which is the reported defect.
    ///
    /// Asserted twice, because the margin is **proportional** and `pan_*` is in
    /// world units: the general clearance scales with each axis' half-extent, so
    /// the literal `0.16` the presets pan by buys different headroom on a wide
    /// target than on a tall one. The second block pins the family's own number
    /// against the landscape targets it will actually meet. On a portrait target
    /// the x axis is the tight one — `9:16` leaves 0.14 of pan headroom against
    /// that 0.16 — which is a property of `pan_x` being world-space, not something
    /// the domain should distort its shape to paper over.
    #[test]
    fn the_wrap_seam_projects_outside_the_visible_frame() {
        // The family's working range (Plan 0043 Phase 1).
        const ZOOMS: [f32; 4] = [1.0, 1.1, 1.2, 1.3];
        const PAN: f32 = 0.16;
        let headroom = MARGIN - 1.0;

        // General: on any target and anywhere in the working zoom range, each seam
        // clears its frame edge by at least `headroom` of that axis' half-extent.
        for aspect in ASPECTS {
            let (bx, by) = bounds(aspect);
            for zoom in ZOOMS {
                assert!(
                    by * zoom - 1.0 >= headroom - 1e-5,
                    "y seam clears by {:.4}, want >= {headroom:.4} (aspect {aspect:.3}, \
                     zoom {zoom:.2})",
                    by * zoom - 1.0
                );
                assert!(
                    bx * zoom - aspect >= aspect * headroom - 1e-5,
                    "x seam clears by {:.4}, want >= {:.4} (aspect {aspect:.3}, zoom {zoom:.2})",
                    bx * zoom - aspect,
                    aspect * headroom
                );
            }
        }

        // Concrete: the pan the swarm presets reach, against every landscape target.
        for aspect in [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0] {
            let (bx, by) = bounds(aspect);
            for zoom in ZOOMS {
                assert!(
                    by * zoom - PAN > 1.0,
                    "y seam at bound {by:.3}, zoom {zoom:.2}, pan {PAN} projects to {:.3} \
                     — inside the frame",
                    by * zoom - PAN
                );
                assert!(
                    bx * zoom - PAN > aspect,
                    "x seam at bound {bx:.3}, zoom {zoom:.2}, pan {PAN} projects to {:.3} \
                     — inside the half-width {aspect:.3}",
                    bx * zoom - PAN
                );
            }
        }

        // What the margin costs, stated so a change to it is deliberate: the
        // visible fraction of the domain is 1 / MARGIN^2.
        let visible = 1.0 / (MARGIN * MARGIN);
        assert!(
            (0.5..0.85).contains(&visible),
            "a margin keeping under half the particles on screen is too expensive: {visible:.3}"
        );
    }

    /// A resize rescales the field instead of teleporting it (ADR-0044's
    /// consequence: "the wrap must stay stable across one rather than teleporting
    /// every particle at once").
    ///
    /// Normalized storage is what buys this, and the test says so by measuring the
    /// alternative alongside: with world-space positions, shrinking the domain
    /// re-wraps everything outside the new bounds, and those particles jump by a
    /// full domain width. Normalized positions move continuously with the change.
    #[test]
    fn a_resize_rescales_the_field_rather_than_wrapping_it() {
        let (before, after) = (16.0 / 9.0, 16.0 / 10.0);
        let (bx0, by0) = bounds(before);
        let (bx1, by1) = bounds(after);

        // A fan of normalized positions spanning the torus, including both seams.
        let samples: Vec<[f32; 2]> = (0..64)
            .map(|i| {
                let u = i as f32 / 63.0 * 2.0 - 1.0;
                [u, -u]
            })
            .collect();

        let mut worst_normalized = 0.0f32;
        let mut worst_world_space = 0.0f32;
        for s in &samples {
            // What this scene does: the normalized position is untouched, so the
            // world position moves by exactly the change in the half-extents.
            let moved =
                ((s[0] * bx1 - s[0] * bx0).powi(2) + (s[1] * by1 - s[1] * by0).powi(2)).sqrt();
            worst_normalized = worst_normalized.max(moved);

            // What a world-space store would do: keep the world position and
            // re-wrap it into the new domain.
            let (mut wx, wy) = (s[0] * bx0, s[1] * by0);
            if wx > bx1 {
                wx -= 2.0 * bx1;
            } else if wx < -bx1 {
                wx += 2.0 * bx1;
            }
            let jump = ((wx - s[0] * bx0).powi(2) + (wy - s[1] * by0).powi(2)).sqrt();
            worst_world_space = worst_world_space.max(jump);
        }

        // 16:9 -> 16:10 narrows the x half-extent by ~0.22 world units; nothing
        // moves further than that, and the y axis does not move at all.
        assert!(
            worst_normalized < 0.3,
            "a resize must move particles continuously, worst was {worst_normalized:.3}"
        );
        assert!(
            worst_world_space > 2.0,
            "the world-space alternative must genuinely teleport, or this test proves \
             nothing: worst jump was {worst_world_space:.3}"
        );
    }

    /// The default hue band (`center = 0.5`, `spread = 1`, `hue = 0`) reduces to
    /// `particle_hue`, so the swarm's colour is unchanged from before Plan 0020.
    #[test]
    fn default_hue_band_is_the_prior_full_wheel() {
        for &ph in &[0.0, 0.2, 0.5, 0.73, 0.99] {
            let coord = hue_coord(DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, ph, DEFAULT_HUE);
            assert!(
                (coord - ph).abs() < 1e-6,
                "default band maps particle_hue to itself: {coord} vs {ph}"
            );
        }
    }

    /// A narrow `hue_spread` collapses the full particle-hue range into a tight
    /// LUT band, so the sampled colours cluster (a coherent single-family swarm)
    /// where `spread = 1` samples the whole wheel (rainbow confetti). Measured as
    /// the spread of sampled RGB — the gap the plan closes.
    #[test]
    fn narrow_spread_makes_colour_coherent() {
        let pal = Palette::default_spectrum();
        // Total variance of the sampled colours across a fan of particle hues.
        let colour_spread = |spread: f32| -> f32 {
            let hues: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
            let cols: Vec<[f32; 3]> = hues
                .iter()
                .map(|&h| pal.sample(hue_coord(0.5, spread, h, 0.0), 0.0))
                .collect();
            let n = cols.len() as f32;
            let mut mean = [0.0f32; 3];
            for c in &cols {
                for k in 0..3 {
                    mean[k] += c[k] / n;
                }
            }
            let mut var = 0.0f32;
            for c in &cols {
                for k in 0..3 {
                    var += (c[k] - mean[k]).powi(2);
                }
            }
            var / n
        };
        let narrow = colour_spread(0.1);
        let full = colour_spread(1.0);
        assert!(
            narrow < full * 0.25,
            "narrow band ({narrow:.4}) is far more coherent than the full wheel ({full:.4})"
        );
    }
}
