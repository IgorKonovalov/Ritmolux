//! The converted-shader runtime (Plan 0100 Phase 6): the GPU half of
//! [`milk::shader`](crate::milk::shader)'s contract.
//!
//! Everything here exists **only for a bundle that carries WGSL**. A native
//! `warp_mesh` preset — and a converted one without shaders — builds none of
//! it, which is what keeps every existing golden byte-identical: an extra
//! device allocation changes what a later pass resolves to on WARP
//! (`core/tests/composite.rs`'s recorded hazard), so the price is only paid by
//! the presets that need it.
//!
//! # What the surface supplies
//!
//! - **The field** as `t_main`, through four samplers (filter/point ×
//!   wrap/clamp) — the reference's own quartet.
//! - **Six procedural noise textures** (`noise_lq`, `_lq_lite`, `_mq`, `_hq`,
//!   and the two 3D volumes). **51 % of the corpus samples one.** MilkDrop
//!   generates them from a seeded RNG at startup; these are generated the same
//!   way, from a *fixed* seed, so every run and every machine sees the same
//!   noise (ADR-0051's determinism rule with the per-run opt-out irrelevant —
//!   the reference never varied them either).
//! - **A three-level blur chain** behind `GetBlur1..3` — **71 % of the corpus
//!   reads one**, which is what makes this a fixture rather than a corner. Each
//!   level is a separable 9-tap gaussian at half the previous level's
//!   resolution, computed once per frame from the freshly-composed field.
//! - **The uniform block** — clock, bands, q's, roam vectors, hue corners, the
//!   `rot_*` rows — filled per frame by [`fill_uniform`].

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Encodes passes and fills a uniform every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::milk::shader::{COMP_GROUP, ROT_MATRICES, WARP_GROUP};
use crate::render::feedback::PingPongField;
use crate::render::gpu;

use super::FIELD_FORMAT;

// The pipeline layouts below place the shader surface where the emitted
// modules bind it — a drift here fails at pipeline creation, so it is pinned at
// compile time instead.
const _: () = assert!(
    WARP_GROUP == 1,
    "the warp module binds its surface at group 1"
);
const _: () = assert!(
    COMP_GROUP == 0,
    "the comp module binds its surface at group 0"
);

/// What a bundle asked the scene to build, extracted at `configure` so the
/// render path never touches the bundle itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ShaderSpec {
    /// The warp fragment module, replacing the built-in decay fragment.
    pub warp: Option<String>,
    /// The comp fragment module, replacing the built-in present fragment.
    pub comp: Option<String>,
    /// How deep the blur chain runs, `0..=3`.
    pub blur: u8,
}

impl ShaderSpec {
    /// A stable key for staleness checks — two presets with different shaders
    /// must rebuild the pipelines.
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.warp.hash(&mut hasher);
        self.comp.hash(&mut hasher);
        self.blur.hash(&mut hasher);
        hasher.finish()
    }
}

/// The uniform block. **Field-for-field with
/// [`UNIFORM_WGSL`](crate::milk::shader::UNIFORM_WGSL)** — every member is
/// 16-byte data, so `#[repr(C)]` and WGSL agree by construction.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MilkUniform {
    clock: [f32; 4],
    bands: [f32; 4],
    bands_att: [f32; 4],
    texsize: [f32; 4],
    aspect: [f32; 4],
    rand_frame: [f32; 4],
    rand_preset: [f32; 4],
    misc: [f32; 4],
    hue: [[f32; 4]; 4],
    q: [[f32; 4]; 8],
    roam: [[f32; 4]; 4],
    rot: [[f32; 4]; ROT_MATRICES * 4],
}

/// Fill the uniform from this frame's runtime state.
///
/// `decay` is the scene's per-second value; it is converted to *this frame's*
/// factor here (`^dt`), so a shader's `ret *= decay` fades at the authored rate
/// on any refresh — ADR-0019 applied to a shader input.
///
/// `quantize_steps` rides the free `misc.w` lane to the emitted warp epilogue's
/// `lmv_quantize` call (ADR-0118). It is a **runtime** input rather than a baked
/// constant precisely so a bundle can be A/B'd without a re-convert.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_uniform(
    runtime: &crate::milk::MilkRuntime,
    time: f32,
    dt: f32,
    size: (u32, u32),
    aspect: f32,
    decay_per_second: f32,
    brightness: f32,
    occlude: f32,
    quantize_steps: f32,
) -> MilkUniform {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    // The EEL convention (`MilkRuntime::run_frame`): the longer axis reads 1.
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let (ax, ay) = if aspect >= 1.0 {
        (1.0, aspect)
    } else {
        (1.0 / aspect.max(1e-4), 1.0)
    };
    let bands = runtime.shader_bands();
    let q = runtime.q_values();
    let mut q_packed = [[0.0f32; 4]; 8];
    for (i, value) in q.iter().enumerate() {
        if let Some(slot) = q_packed.get_mut(i / 4).and_then(|row| row.get_mut(i % 4)) {
            *slot = *value;
        }
    }
    let band = |i: usize| bands.get(i).copied().unwrap_or(0.0);
    MilkUniform {
        clock: [
            time,
            crate::milk::NOMINAL_FPS,
            runtime.frame_index() as f32,
            0.0,
        ],
        bands: [band(0), band(1), band(2), band(3)],
        bands_att: [band(4), band(5), band(6), band(7)],
        texsize: [w, h, 1.0 / w, 1.0 / h],
        aspect: [ax, ay, 1.0 / ax, 1.0 / ay],
        rand_frame: runtime.rand_frame(),
        rand_preset: runtime.rand_preset(),
        misc: [
            decay_per_second.max(0.0).powf(dt.max(1e-6)),
            brightness,
            occlude,
            quantize_steps,
        ],
        hue: hue_corners(time),
        q: q_packed,
        roam: roam_vectors(time),
        rot: rot_rows(time, runtime.salt()),
    }
}

/// The four corner colours `hue_shader` interpolates: slowly drifting, always
/// bright (the max channel is normalized to 1, as the reference normalizes its
/// hue palette). Pure in `time`, so a capture reproduces.
fn hue_corners(time: f32) -> [[f32; 4]; 4] {
    std::array::from_fn(|k| {
        let k = k as f32;
        let r = 0.6 + 0.3 * (time * 0.0417 + 1.0 + k * 2.1).sin();
        let g = 0.6 + 0.3 * (time * 0.0722 + 4.0 + k * 1.3).sin();
        let b = 0.6 + 0.3 * (time * 0.0913 + 2.0 + k * 3.7).sin();
        let peak = r.max(g).max(b).max(1e-3);
        [r / peak, g / peak, b / peak, 1.0]
    })
}

/// `slow_roam_cos`, `roam_cos`, `slow_roam_sin`, `roam_sin` — the reference's
/// own frequencies (0.005/0.008/0.013/0.022 and 0.3/1.3/5/20), remapped to
/// `0..1` exactly as it remaps them.
fn roam_vectors(time: f32) -> [[f32; 4]; 4] {
    let slow = [0.005, 0.008, 0.013, 0.022];
    let fast = [0.3, 1.3, 5.0, 20.0];
    let wave = |rates: [f32; 4], f: fn(f32) -> f32| -> [f32; 4] {
        std::array::from_fn(|i| 0.5 + 0.5 * f(time * rates.get(i).copied().unwrap_or(1.0)))
    };
    [
        wave(slow, f32::cos),
        wave(fast, f32::cos),
        wave(slow, f32::sin),
        wave(fast, f32::sin),
    ]
}

/// The 24 `rot_*` matrices as 96 `vec4` rows: a slow rotation about a
/// salt-seeded axis per matrix, families `s`→`rand` spinning progressively
/// faster, with a drifting translation in the fourth row.
///
/// **0.27 % of the corpus reads one** (28 files, mostly a single row component
/// as a smooth pseudo-random), so this is a faithful *gesture* — smooth, seeded,
/// family-rated — rather than a bit-exact port of the reference's generator.
fn rot_rows(time: f32, salt: u32) -> [[f32; 4]; ROT_MATRICES * 4] {
    let mut rows = [[0.0f32; 4]; ROT_MATRICES * 4];
    let family_rate = [0.05, 0.2, 0.5, 1.0, 2.0, 3.0];
    for m in 0..ROT_MATRICES {
        let h = |k: u32| -> f32 {
            let mixed = mix32(salt ^ (m as u32).wrapping_mul(0x9E37_79B9).wrapping_add(k));
            (mixed >> 8) as f32 / 16_777_216.0
        };
        // A unit axis and a family-rated spin.
        let (x, y, z) = (h(1) * 2.0 - 1.0, h(2) * 2.0 - 1.0, h(3) * 2.0 - 1.0);
        let len = (x * x + y * y + z * z).sqrt().max(1e-3);
        let (x, y, z) = (x / len, y / len, z / len);
        let rate = family_rate.get(m / 4).copied().unwrap_or(1.0) * (0.7 + 0.6 * h(4));
        let angle = time * rate + h(5) * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        let t = 1.0 - c;
        // Rodrigues' rotation, row-major.
        let r = [
            [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
        ];
        for (i, row) in r.iter().enumerate() {
            if let Some(slot) = rows.get_mut(m * 4 + i) {
                *slot = [
                    row.first().copied().unwrap_or(0.0),
                    row.get(1).copied().unwrap_or(0.0),
                    row.get(2).copied().unwrap_or(0.0),
                    0.0,
                ];
            }
        }
        if let Some(slot) = rows.get_mut(m * 4 + 3) {
            let tau = std::f32::consts::TAU;
            *slot = [
                (time * rate * 0.31 + h(6) * tau).sin() * 0.5,
                (time * rate * 0.23 + h(7) * tau).sin() * 0.5,
                (time * rate * 0.17 + h(8) * tau).sin() * 0.5,
                1.0,
            ];
        }
    }
    rows
}

fn mix32(v: u32) -> u32 {
    let mut h = v;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

// ---------------------------------------------------------------------------
// Procedural noise
// ---------------------------------------------------------------------------

/// The fixed seed every noise texture generates from. A constant, not the
/// preset salt: MilkDrop builds its noise once at startup and every preset
/// shares it, so two presets sampling `noise_lq` see the same texture.
const NOISE_SEED: u32 = 0x4C4D_5601;

/// RGBA8 value noise: at `zoom = 1`, pure per-texel randoms; above it, a
/// `size/zoom` random lattice smoothly interpolated up (wrapping), which is the
/// reference's own two shapes (`noise_lq` vs `noise_mq`/`_hq`).
fn noise_2d(size: u32, zoom: u32, seed: u32) -> Vec<u8> {
    let size = size.max(1) as usize;
    let zoom = zoom.max(1) as usize;
    let cell = |x: usize, y: usize, c: usize| -> f32 {
        let h = mix32(
            seed ^ mix32(
                (x as u32).wrapping_mul(0x0068_9AF5) ^ (y as u32).wrapping_mul(0x0233_58E1),
            )
            .wrapping_add(c as u32),
        );
        (h >> 8) as f32 / 16_777_216.0
    };
    let base = size / zoom;
    let mut out = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            for c in 0..4 {
                let v = if zoom == 1 {
                    cell(x, y, c)
                } else {
                    let fx = x as f32 / zoom as f32;
                    let fy = y as f32 / zoom as f32;
                    let (x0, y0) = (fx as usize % base.max(1), fy as usize % base.max(1));
                    let (x1, y1) = ((x0 + 1) % base.max(1), (y0 + 1) % base.max(1));
                    let (tx, ty) = (smooth(fx.fract()), smooth(fy.fract()));
                    let a = cell(x0, y0, c) * (1.0 - tx) + cell(x1, y0, c) * tx;
                    let b = cell(x0, y1, c) * (1.0 - tx) + cell(x1, y1, c) * tx;
                    a * (1.0 - ty) + b * ty
                };
                out.push((v * 255.0) as u8);
            }
        }
    }
    out
}

/// The 3D volumes, same construction as [`noise_2d`].
fn noise_3d(size: u32, zoom: u32, seed: u32) -> Vec<u8> {
    let size = size.max(1) as usize;
    let zoom = zoom.max(1) as usize;
    let cell = |x: usize, y: usize, z: usize, c: usize| -> f32 {
        let h = mix32(
            seed ^ mix32(
                (x as u32).wrapping_mul(0x0068_9AF5)
                    ^ (y as u32).wrapping_mul(0x0233_58E1)
                    ^ (z as u32).wrapping_mul(0x0741_2913),
            )
            .wrapping_add(c as u32),
        );
        (h >> 8) as f32 / 16_777_216.0
    };
    let base = (size / zoom).max(1);
    let mut out = Vec::with_capacity(size * size * size * 4);
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                for c in 0..4 {
                    let v = if zoom == 1 {
                        cell(x, y, z, c)
                    } else {
                        let (fx, fy, fz) = (
                            x as f32 / zoom as f32,
                            y as f32 / zoom as f32,
                            z as f32 / zoom as f32,
                        );
                        let (x0, y0, z0) =
                            (fx as usize % base, fy as usize % base, fz as usize % base);
                        let (x1, y1, z1) = ((x0 + 1) % base, (y0 + 1) % base, (z0 + 1) % base);
                        let (tx, ty, tz) =
                            (smooth(fx.fract()), smooth(fy.fract()), smooth(fz.fract()));
                        let lerp = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
                        let c00 = lerp(cell(x0, y0, z0, c), cell(x1, y0, z0, c), tx);
                        let c10 = lerp(cell(x0, y1, z0, c), cell(x1, y1, z0, c), tx);
                        let c01 = lerp(cell(x0, y0, z1, c), cell(x1, y0, z1, c), tx);
                        let c11 = lerp(cell(x0, y1, z1, c), cell(x1, y1, z1, c), tx);
                        lerp(lerp(c00, c10, ty), lerp(c01, c11, ty), tz)
                    };
                    out.push((v * 255.0) as u8);
                }
            }
        }
    }
    out
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// The blur chain
// ---------------------------------------------------------------------------

/// One separable gaussian tap pass. The weights sum to 1; the shape is a plain
/// sigma≈2 gaussian — the reference's blur is the same gesture with adjustable
/// range, which no preset in scope adjusts.
const BLUR_SHADER: &str = r#"
struct BlurU {
    // xy: one source texel along the blur direction, in uv
    step: vec4<f32>,
}
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> bu: BlurU;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = bu.step.xy;
    var acc = textureSampleLevel(src, src_samp, in.uv, 0.0).rgb * 0.2026;
    acc = acc + (textureSampleLevel(src, src_samp, in.uv + d, 0.0).rgb
               + textureSampleLevel(src, src_samp, in.uv - d, 0.0).rgb) * 0.1790;
    acc = acc + (textureSampleLevel(src, src_samp, in.uv + d * 2.0, 0.0).rgb
               + textureSampleLevel(src, src_samp, in.uv - d * 2.0, 0.0).rgb) * 0.1240;
    acc = acc + (textureSampleLevel(src, src_samp, in.uv + d * 3.0, 0.0).rgb
               + textureSampleLevel(src, src_samp, in.uv - d * 3.0, 0.0).rgb) * 0.0672;
    acc = acc + (textureSampleLevel(src, src_samp, in.uv + d * 4.0, 0.0).rgb
               + textureSampleLevel(src, src_samp, in.uv - d * 4.0, 0.0).rgb) * 0.0285;
    return vec4<f32>(acc, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurUniform {
    step: [f32; 4],
}

/// One encoded blur pass: its target view and its bind group. The first pass's
/// group is picked per frame by which half of the field is being read.
struct BlurPass {
    target: wgpu::TextureView,
    /// Bind group reading the fixed source (`None` for the first pass).
    bind: Option<wgpu::BindGroup>,
    /// The first pass's two variants, reading field A / field B.
    first: Option<(wgpu::BindGroup, wgpu::BindGroup)>,
}

/// The chain: `field → temp1 → blur1 → temp2 → blur2 → temp3 → blur3`, each
/// level at half the previous resolution, H then V.
struct BlurChain {
    pipeline: wgpu::RenderPipeline,
    passes: Vec<BlurPass>,
    /// The blur textures themselves, viewed by the shader-surface bind groups.
    views: Vec<wgpu::TextureView>,
    /// Kept alive for the views' sake.
    _textures: Vec<wgpu::Texture>,
    _uniforms: Vec<wgpu::Buffer>,
}

// ---------------------------------------------------------------------------
// The whole shader surface
// ---------------------------------------------------------------------------

/// Everything a shader-carrying bundle adds to [`super::Resources`].
pub(super) struct MilkShaderResources {
    /// The custom warp pipeline, when the bundle carries a warp shader.
    pub warp_pipeline: Option<wgpu::RenderPipeline>,
    /// The custom comp pipeline, replacing the built-in present.
    pub comp_pipeline: Option<wgpu::RenderPipeline>,
    /// The shader surface reading field A / field B as `t_main`.
    pub bind_a: wgpu::BindGroup,
    pub bind_b: wgpu::BindGroup,
    /// The uniform buffer behind `U`.
    pub uniform: wgpu::Buffer,
    blur: Option<BlurChain>,
    _noise: Vec<wgpu::Texture>,
}

impl MilkShaderResources {
    /// Build the surface for `spec`. Runs at preset switch / resize, never per
    /// frame. `queue` uploads the generated noise once.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        spec: &ShaderSpec,
        field: &PingPongField,
        size: (u32, u32),
        surface_format: wgpu::TextureFormat,
        warp_vs: &wgpu::ShaderModule,
        warp_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // --- samplers: the reference's quartet ---
        let sampler = |label: &str, filter: wgpu::FilterMode, address: wgpu::AddressMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: address,
                address_mode_v: address,
                address_mode_w: address,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let s_fw = sampler(
            "warp-mesh-milk-fw",
            wgpu::FilterMode::Linear,
            wgpu::AddressMode::Repeat,
        );
        let s_fc = sampler(
            "warp-mesh-milk-fc",
            wgpu::FilterMode::Linear,
            wgpu::AddressMode::ClampToEdge,
        );
        let s_pw = sampler(
            "warp-mesh-milk-pw",
            wgpu::FilterMode::Nearest,
            wgpu::AddressMode::Repeat,
        );
        let s_pc = sampler(
            "warp-mesh-milk-pc",
            wgpu::FilterMode::Nearest,
            wgpu::AddressMode::ClampToEdge,
        );

        // --- the noise set, generated once from the fixed seed ---
        let mut noise_textures = Vec::new();
        let mut noise_views = Vec::new();
        let queue_data: [(&str, u32, u32, bool); 6] = [
            ("noise_lq", 256, 1, false),
            ("noise_lq_lite", 32, 1, false),
            ("noise_mq", 256, 4, false),
            ("noise_hq", 256, 8, false),
            ("noisevol_lq", 32, 1, true),
            ("noisevol_hq", 32, 4, true),
        ];
        for (name, tex_size, zoom, volume) in queue_data {
            let (data, extent, dimension) = if volume {
                (
                    noise_3d(tex_size, zoom, NOISE_SEED ^ zoom),
                    wgpu::Extent3d {
                        width: tex_size,
                        height: tex_size,
                        depth_or_array_layers: tex_size,
                    },
                    wgpu::TextureDimension::D3,
                )
            } else {
                (
                    noise_2d(tex_size, zoom, NOISE_SEED.wrapping_add(zoom)),
                    wgpu::Extent3d {
                        width: tex_size,
                        height: tex_size,
                        depth_or_array_layers: 1,
                    },
                    wgpu::TextureDimension::D2,
                )
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("warp-mesh-milk-{name}")),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tex_size * 4),
                    rows_per_image: Some(tex_size),
                },
                extent,
            );
            noise_views.push(texture.create_view(&wgpu::TextureViewDescriptor::default()));
            noise_textures.push(texture);
        }

        // --- the blur chain (or three 1x1 stand-ins when unused) ---
        let blur = (spec.blur > 0).then(|| Self::build_blur(device, field, size, spec.blur, &s_fc));
        // Bindings 12..=14 are always three deep: levels the chain never
        // computes are 1x1 stand-ins, never sampled (an entry point that never
        // reaches `GetBlurN` never puts its binding in the resource set) but
        // required by the fixed layout shape.
        let mut dummy_textures = Vec::new();
        let mut blur_views: Vec<wgpu::TextureView> = match &blur {
            Some(chain) => chain.views.iter().map(clone_view).collect(),
            None => Vec::new(),
        };
        for i in blur_views.len()..3 {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("warp-mesh-milk-blur-dummy-{i}")),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FIELD_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            dummy_textures.push(texture);
            blur_views.push(view);
        }
        noise_textures.extend(dummy_textures);

        // --- the surface layout, positionally `milk::shader::BINDINGS` ---
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-milk-uniform"),
            size: std::mem::size_of::<MilkUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Fifteen entries — a shape nothing else in the crate approaches in
        // length, which is its ADR-0058 separation.
        let texture_3d = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D3,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("warp-mesh-milk-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<MilkUniform>() as u64,
                        ),
                    },
                    count: None,
                },
                gpu::texture(1, true),
                gpu::sampler(2),
                gpu::sampler(3),
                gpu::sampler(4),
                gpu::sampler(5),
                gpu::texture(6, true),
                gpu::texture(7, true),
                gpu::texture(8, true),
                gpu::texture(9, true),
                texture_3d(10),
                texture_3d(11),
                gpu::texture(12, true),
                gpu::texture(13, true),
                gpu::texture(14, true),
            ],
        });
        let bind = |main_view: &wgpu::TextureView| {
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(main_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&s_fw),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&s_fc),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&s_pw),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&s_pc),
                },
            ];
            // The noise set fills 6..=11 and the blur levels 12..=14, in
            // `milk::shader::BINDINGS` order.
            for (i, view) in noise_views.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: (6 + i) as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                });
            }
            for (i, view) in blur_views.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: (12 + i) as u32,
                    resource: wgpu::BindingResource::TextureView(view),
                });
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("warp-mesh-milk-bg"),
                layout: &layout,
                entries: &entries,
            })
        };
        let bind_a = bind(field.view_a());
        let bind_b = bind(field.view_b());

        // --- the two custom pipelines ---
        let warp_pipeline = spec.warp.as_deref().map(|wgsl| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("warp-mesh-milk-warp-fs"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("warp-mesh-milk-warp-pipeline-layout"),
                bind_group_layouts: &[Some(warp_layout), Some(&layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("warp-mesh-milk-warp-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: warp_vs,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<super::Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &super::VERTEX_ATTRS,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FIELD_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        let comp_pipeline = spec.comp.as_deref().map(|wgsl| {
            let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("warp-mesh-milk-comp-vs"),
                source: wgpu::ShaderSource::Wgsl(gpu::FULLSCREEN_VS_UV_FLIPPED.into()),
            });
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("warp-mesh-milk-comp-fs"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("warp-mesh-milk-comp-pipeline-layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("warp-mesh-milk-comp-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vs,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        // Premultiplied OVER the backdrop, exactly as the
                        // built-in present composes (ADR-0026).
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });

        Self {
            warp_pipeline,
            comp_pipeline,
            bind_a,
            bind_b,
            uniform,
            blur,
            _noise: noise_textures,
        }
    }

    fn build_blur(
        device: &wgpu::Device,
        field: &PingPongField,
        size: (u32, u32),
        levels: u8,
        sampler: &wgpu::Sampler,
    ) -> BlurChain {
        let shader = gpu::fullscreen_shader(
            device,
            "warp-mesh-milk-blur-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            BLUR_SHADER,
        );
        // Texture first, uniform last **and sized**: distinct from
        // `warp-mesh-present-layout`'s `[Uniform, Texture, Sampler]` and from
        // `warp-mesh-deposit-layout`'s `[T, T, S, U]` (ADR-0058).
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("warp-mesh-milk-blur-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::sampler(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<BlurUniform>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&layout],
            FIELD_FORMAT,
            wgpu::BlendState::REPLACE,
            "warp-mesh-milk-blur",
        );

        let mut textures = Vec::new();
        let mut views = Vec::new();
        let mut temp_views = Vec::new();
        let mut level_sizes = Vec::new();
        for level in 1..=levels.min(3) as u32 {
            let (w, h) = ((size.0 >> level).max(1), (size.1 >> level).max(1));
            level_sizes.push((w, h));
            for kind in ["blur", "blur-temp"] {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("warp-mesh-milk-{kind}{level}")),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FIELD_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                if kind == "blur" {
                    views.push(view);
                } else {
                    temp_views.push(view);
                }
                textures.push(texture);
            }
        }

        // Six passes at most: per level, H into temp then V into the level.
        let mut uniforms = Vec::new();
        let mut passes = Vec::new();
        for (index, (_, level_h)) in level_sizes.iter().copied().enumerate() {
            // The source of the H pass: the field for level 1, the previous
            // blur level after.
            let src_size = if index == 0 {
                size
            } else {
                level_sizes.get(index - 1).copied().unwrap_or(size)
            };
            let step_uniform = |dx: f32, dy: f32| {
                use wgpu::util::DeviceExt as _;
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("warp-mesh-milk-blur-step"),
                    contents: bytemuck::bytes_of(&BlurUniform {
                        step: [dx, dy, 0.0, 0.0],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM,
                })
            };
            let h_uniform = step_uniform(1.0 / src_size.0.max(1) as f32, 0.0);
            let v_uniform = step_uniform(0.0, 1.0 / level_h.max(1) as f32);
            let bind_of = |view: &wgpu::TextureView, uniform: &wgpu::Buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("warp-mesh-milk-blur-bg"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform.as_entire_binding(),
                        },
                    ],
                })
            };
            let Some(temp_view) = temp_views.get(index) else {
                continue;
            };
            let Some(level_view) = views.get(index) else {
                continue;
            };
            // H pass.
            if index == 0 {
                passes.push(BlurPass {
                    target: clone_view(temp_view),
                    bind: None,
                    first: Some((
                        bind_of(field.view_a(), &h_uniform),
                        bind_of(field.view_b(), &h_uniform),
                    )),
                });
            } else if let Some(previous) = views.get(index - 1) {
                passes.push(BlurPass {
                    target: clone_view(temp_view),
                    bind: Some(bind_of(previous, &h_uniform)),
                    first: None,
                });
            }
            // V pass.
            passes.push(BlurPass {
                target: clone_view(level_view),
                bind: Some(bind_of(temp_view, &v_uniform)),
                first: None,
            });
            uniforms.push(h_uniform);
            uniforms.push(v_uniform);
        }

        BlurChain {
            pipeline,
            passes,
            views,
            _textures: textures,
            _uniforms: uniforms,
        }
    }

    /// Clear every blur target once — their contents are undefined until
    /// written, and a custom warp shader may `GetBlur` on the very first frame,
    /// before the first chain has run.
    pub fn encode_clear(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(chain) = &self.blur else {
            return;
        };
        for pass in &chain.passes {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp-mesh-milk-blur-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &pass.target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }

    /// Encode the blur chain, reading the field's current read side. A no-op
    /// when the bundle never blurs.
    pub fn encode_blur(&self, encoder: &mut wgpu::CommandEncoder, reading_a: bool) {
        let Some(chain) = &self.blur else {
            return;
        };
        for pass in &chain.passes {
            let bind = match (&pass.bind, &pass.first) {
                (Some(bind), _) => bind,
                (None, Some((a, b))) => {
                    if reading_a {
                        a
                    } else {
                        b
                    }
                }
                (None, None) => continue,
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp-mesh-milk-blur-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &pass.target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&chain.pipeline);
            rp.set_bind_group(0, bind, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

/// wgpu texture views are `Arc`-backed; cloning one is a refcount bump.
fn clone_view(view: &wgpu::TextureView) -> wgpu::TextureView {
    view.clone()
}
