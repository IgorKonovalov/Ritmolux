//! Fragment-field scene: a fullscreen Shadertoy-style domain-warped field,
//! colored through the shared palette LUT (ADR-0021). The first "generative-art"-
//! tier built-in and one of the two preset-driven systems (ADR-0002 layers 1-2).
//!
//! Its look is a set of named parameters — `warp`, `hue`, `zoom`, `glow`,
//! `flash`, plus the shared color knobs `color_span`/`color_center`/`saturation`
//! (Plan 0020) — that a preset binds to expressions over the audio analysis (Plan
//! 0003 Phase 5). With no preset the parameter defaults render a gentle idle
//! field. The scene reads no audio directly; all reactivity flows through the
//! parameter values.
//!
//! Color: the field level indexes a 256-entry gradient LUT (the preset's
//! `[palette]`, default `spectrum` = the exact prior cosine) instead of a
//! hardcoded `palette()`. `color_span` sets how much of the gradient the field
//! spans (replacing the old fixed `field*0.6`), `color_center`/`hue` slide the
//! window, and `saturation` desaturates toward luma — all bindable.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;

use super::Scene;
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

/// Parameter defaults — a calm idle field when nothing is bound.
const DEFAULT_WARP: f32 = 0.4;
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_GLOW: f32 = 0.7;
const DEFAULT_FLASH: f32 = 0.0;
// Shared view transform (ADR-0018): `pan_*` offset the sampled field window. The
// field's existing `zoom` already scales the sample coordinates (its view-zoom in
// field space), so Phase 2 completes the ViewTransform here by adding pan.
const DEFAULT_PAN: f32 = 0.0;
// Shared palette color knobs (ADR-0021). `color_span` = 0.6 + `color_center` = 0
// + `saturation` = 1 reproduce the prior look exactly (the old `field*0.6` sample
// with no desaturation).
const DEFAULT_COLOR_SPAN: f32 = 0.6;
const DEFAULT_COLOR_CENTER: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only (a no-op unless a preset declares
/// `[palette_b]` and binds `palette_mix`).
const DEFAULT_PALETTE_MIX: f32 = 0.0;
/// The two rate parameters (ADR-0132), in units of the scene's own default
/// speed. `1.0` is what this scene has always animated at, so a preset that
/// binds neither renders exactly as before.
const DEFAULT_FIELD_SPEED: f32 = 1.0;
const DEFAULT_FOLD_SPEED: f32 = 1.0;

const SHADER: &str = r#"
struct Params {
    // x: time (s), y: aspect, z: warp, w: hue
    a: vec4<f32>,
    // x: zoom, y: glow, z: flash, w: color_span
    b: vec4<f32>,
    // xy: pan (field-space offset, ADR-0018), z: color_center, w: saturation
    c: vec4<f32>,
    // x: palette_mix (A/B crossfade), y: occlude (ADR-0085),
    // z: palette_steps (integral, quantized CPU-side), w: palette_contour (ADR-0078)
    d: vec4<f32>,
    // x: fold phase, y: field phase (ADR-0132) — both INTEGRATED on the CPU
    // (`phase += rate * dt`) rather than derived here from `t * rate`. At a
    // constant rate a phase equals `rate * t`, so the defaults reproduce the
    // literals these replaced; what integration buys is that a rate BOUND TO
    // AUDIO bends the motion instead of teleporting it — at t = 100 s a
    // `warp_speed`-style multiply would move the phase fifty seconds in one
    // frame. z, w: unused.
    e: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
// The gradient LUTs sit in their own bind group (group 1), so this pipeline's
// layout stays distinct from the screen-space kaleidoscope's single 3-entry
// [uniform, texture, sampler] group — two byte-identical layouts mis-render when
// they coexist on the DX12 WARP software adapter (the same quirk the shared line
// renderer and the lazy feedback scenes work around). Two LUTs (A/B) for the
// `palette_mix` crossfade; one shared sampler.
@group(1) @binding(0) var lut_a: texture_2d<f32>;
@group(1) @binding(1) var lut_b: texture_2d<f32>;
@group(1) @binding(2) var lut_samp: sampler;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim):
// scale chroma around Rec. 601 luma. 1.0 unchanged, 0.0 grayscale.
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord
// verbatim, ADR-0078): snap the palette coordinate to a band centre before the
// LUT read. Below 1.5 steps it is the exact identity, not a one-band degenerate.
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Shared `palette_contour` (ADR-0078 / ADR-0133; the WGSL is the implementation,
// copied verbatim at each fragment-stage site — palette.rs has no CPU
// counterpart to be canonical, since `fwidth` exists only here).
//
// Darkens within one PIXEL of a band edge, so the line has the same weight where
// the field is shallow and where it is steep — AND ONLY WHERE THE INK ACTUALLY
// CHANGES (ADR-0133). It samples the two band centres either side of the nearest
// edge and returns unchanged when they resolve to the same colour within half a
// code value, which is below the LUT's own 8-bit quantization. On a smooth
// palette two distinct centres always differ by at least one code value, so
// every edge draws exactly as it did at any `palette_steps`; inside a plateau
// the LUT is literally constant and the samples are bit-equal, so the line
// vanishes there and survives at the run boundaries. One rule, both behaviours,
// no new parameter.
//
// The two LUTs, the sampler and `palette_mix` are EXPLICIT parameters rather
// than module-scope globals this happens to find: all four sites name them the
// same today, so implicit capture would compile — and would silently bind the
// shared function to whatever a future site called its textures.
//
// `textureSampleLevel`, not `textureSample`: the LUT has one mip, and an
// explicit LOD keeps these reads free of the uniformity requirement that a
// sample after a conditional return would otherwise carry.
fn band_contour(
    t: f32,
    steps: f32,
    amount: f32,
    lut_a: texture_2d<f32>,
    lut_b: texture_2d<f32>,
    lut_samp: sampler,
    mix_ab: f32,
) -> f32 {
    let f = t * steps;
    let w = max(fwidth(f), 1e-5);
    if (steps < 1.5 || amount <= 0.0) {
        return 1.0;
    }
    let n = round(f);
    let m = clamp(mix_ab, 0.0, 1.0);
    let lo = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    let hi = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    if (all(abs(hi - lo) < vec3<f32>(0.5 / 255.0))) {
        return 1.0;
    }
    let d = min(fract(f), 1.0 - fract(f));
    return 1.0 - clamp(amount, 0.0, 1.0) * (1.0 - smoothstep(0.0, w, d));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = params.a.x;
    let aspect = params.a.y;
    let warp = params.a.z;
    let hue = params.a.w;
    let zoom = params.b.x;
    let glow = params.b.y;
    let flash = params.b.z;
    let color_span = params.b.w;
    let pan = params.c.xy;
    let color_center = params.c.z;
    let saturation = params.c.w;
    let palette_mix = params.d.x;
    let palette_steps = params.d.z;
    let palette_contour = params.d.w;
    let fold_phase = params.e.x;
    let field_phase = params.e.y;

    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // Iterated sine-fold domain warp, scaled by zoom and folded by warp; `pan`
    // slides the sampled field window (the shared ViewTransform, ADR-0018). The
    // vignette below stays screen-anchored (uses unshifted `uv`).
    var p = uv * zoom + pan;
    // The fold's two rates keep their designed 0.7 : 0.6 quadrature ratio; what
    // `fold_speed` scales is the phase they share, so slowing the fold does not
    // flatten it the way `warp` does (ADR-0132).
    for (var i = 0; i < 5; i = i + 1) {
        let fi = f32(i);
        p = p + warp * vec2<f32>(
            sin(p.y * 1.5 + fold_phase * 0.7 + fi),
            cos(p.x * 1.5 - fold_phase * 0.6 + fi)
        ) / (fi + 1.0);
    }

    let field = 0.5 + 0.5 * sin(p.x + p.y + field_phase * 0.5);
    // Field level indexes the gradient LUT: `color_span` sets the spanned range
    // (was a fixed 0.6), `color_center`/`hue` slide the window. Linear-filtered,
    // repeat-addressed (a hue rotation wraps like the cosine wheel).
    let coord = field * color_span + color_center + hue;
    // Hard bands, then the contour drawn from the SAME coordinate (ADR-0078), so
    // the dark line follows the field's iso-lines and reads as structure rather
    // than as an outline of the picture's brightness.
    let banded = band_coord(coord, palette_steps);
    // Sample both palettes and crossfade by `palette_mix` (0 = A, 1 = B). When a
    // preset declares no [palette_b] the two LUTs are identical, so mix is a no-op.
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = col * band_contour(
        coord, palette_steps, palette_contour, lut_a, lut_b, lut_samp, palette_mix
    );
    col = apply_saturation(col, saturation);

    let r = length(uv);
    col = col * (glow * (1.0 - 0.25 * r));
    col = col + vec3<f32>(flash * 0.12);

    // Alpha 1.0: this field covers every pixel, which is the coverage it honestly
    // has (ADR-0056). `occlude` scales how much of that the backdrop underneath
    // resolves against (ADR-0085) — at 0 the sky adds through an opaque field.
    // Reached only when no post stage is active; the chain owns the seam otherwise
    // and the renderer hands a literal 1.0 here.
    return vec4<f32>(col, params.d.y);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
    e: [f32; 4],
}

/// The scene's two integrated phases (ADR-0132).
///
/// Its own type, and unit-tested without a device, because the property that
/// matters here is arithmetic rather than visual: a phase must advance by
/// `rate * dt` **whatever the elapsed scene time**, which is exactly what
/// `time * rate` fails to do the moment a preset binds the rate to audio.
#[derive(Clone, Copy, Default)]
struct Phases {
    fold: f32,
    field: f32,
}

impl Phases {
    /// One frame's integration, at *this* frame's rates.
    ///
    /// Called from [`Scene::update`], never from `advance` — the per-frame order
    /// is `set_time` → `advance` → `reset_params` → `set_param` → `update`
    /// (`core/src/render/mod.rs`), so integrating in `advance` would use the
    /// previous frame's rate values.
    fn step(&mut self, fold_speed: f32, field_speed: f32, dt: f32) {
        self.fold += fold_speed * dt;
        self.field += field_speed * dt;
    }
}

/// Fullscreen domain-warped fragment field, driven by named preset parameters.
pub struct FragmentFieldScene {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The LUT textures + sampler bind group (group 1), kept distinct from the
    /// uniform group so the pipeline layout does not match the kaleidoscope's.
    lut_bind_group: wgpu::BindGroup,
    /// The 256×1 gradient LUT textures (A/B) the fragment samples + crossfades
    /// for color (ADR-0021).
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    /// The active baked palette pair, re-uploaded to `lut_texture_a`/`_b` when
    /// `palette_dirty` (set by `set_palette` on a preset switch), off the hot path.
    palette: Palette,
    palette_dirty: bool,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    /// This frame's elapsed real time, stored by `advance` and consumed by
    /// `update` — the split ADR-0132 requires, since `advance` runs before this
    /// frame's parameter values land.
    dt: f32,
    /// The integrated fold and field phases. **This is the scene's only state**:
    /// everything else here is derived from `time` and the parameters, so a
    /// capture's reproducibility now depends on the scene being rebuilt with the
    /// preset as well as on the clock resetting (`reset_for_capture` does both).
    phases: Phases,
    /// Animation rates, in units of the scene's default speed (ADR-0132).
    field_speed: f32,
    fold_speed: f32,
    warp: f32,
    hue: f32,
    zoom: f32,
    glow: f32,
    flash: f32,
    pan_x: f32,
    pan_y: f32,
    color_span: f32,
    color_center: f32,
    saturation: f32,
    /// A/B palette crossfade position (Plan 0020 Phase 4); 0 = palette A.
    palette_mix: f32,
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    /// How much of this field's (total) coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame through
    /// [`Scene::set_occlude`](super::Scene::set_occlude) — **not** a named param,
    /// so it is not reset by `reset_params`.
    occlude: f32,
}

impl FragmentFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = gpu::fullscreen_shader(
            device,
            "fragment-field-shader",
            gpu::FULLSCREEN_VS_NDC,
            SHADER,
        );
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fragment-field-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_texture_a = palette::lut_texture(device, "fragment-field-lut-a");
        let lut_texture_b = palette::lut_texture(device, "fragment-field-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fragment-field-uniform-layout"),
            entries: &[gpu::uniform(0, wgpu::ShaderStages::FRAGMENT)],
        });
        // The LUT texture + sampler live in their own group (group 1) — see the
        // WGSL note: keeping this pipeline's layout distinct from the
        // kaleidoscope's avoids the DX12 WARP identical-layout mis-render.
        let lut_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fragment-field-lut-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::texture(1, true),
                gpu::sampler(2),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fragment-field-uniform-bg"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let lut_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fragment-field-lut-bg"),
            layout: &lut_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
            ],
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&uniform_layout, &lut_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "fragment-field",
        );

        Self {
            pipeline,
            uniforms,
            bind_group,
            lut_bind_group,
            lut_texture_a,
            lut_texture_b,
            // Seed with the default `spectrum` (the prior cosine); the renderer
            // calls `set_palette` before the first frame with the active preset's
            // palette, and `render` uploads it. Seeding here keeps the texture
            // valid even if `set_palette` were never called.
            palette: Palette::default_spectrum(),
            palette_dirty: true,
            time: 0.0,
            dt: crate::render::scenes::FALLBACK_DT,
            phases: Phases::default(),
            field_speed: DEFAULT_FIELD_SPEED,
            fold_speed: DEFAULT_FOLD_SPEED,
            warp: DEFAULT_WARP,
            hue: DEFAULT_HUE,
            zoom: DEFAULT_ZOOM,
            glow: DEFAULT_GLOW,
            flash: DEFAULT_FLASH,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
        }
    }
}

/// The parameter names this scene consumes — the vocabulary a preset binding
/// is checked against at load, so a typo is warned about instead of silently
/// doing nothing (ADR-0020). **Keep in sync with `set_param` below**; the
/// `declared_params_match_set_param` guard in `core/tests/preset.rs` fails if
/// the two drift.
pub const PARAMS: &[&str] = &[
    "warp",
    "field_speed",
    "fold_speed",
    "hue",
    "zoom",
    "glow",
    "flash",
    "pan_x",
    "pan_y",
    "color_span",
    "color_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
];

impl Scene for FragmentFieldScene {
    fn name(&self) -> &'static str {
        "fragment field"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn advance(&mut self, dt: f32) {
        // Stored, not integrated: the rates this frame will use have not been
        // set yet (ADR-0132). A non-finite or negative `dt` degrades to the
        // capture step rather than poisoning the accumulators, which are the
        // one piece of state here that a bad frame could corrupt permanently.
        self.dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            crate::render::scenes::FALLBACK_DT
        };
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        // Store the baked LUT; `render` uploads it (deferred so scenes with lazy
        // GPU resources share this seam). Cheap array copy, off the hot path.
        self.palette = palette.clone();
        self.palette_dirty = true;
    }

    fn reset_params(&mut self) {
        // The two rates reset with every other param; the PHASES do not — they
        // are state, and resetting them each frame would be the multiply this
        // ADR exists to remove.
        self.field_speed = DEFAULT_FIELD_SPEED;
        self.fold_speed = DEFAULT_FOLD_SPEED;
        self.warp = DEFAULT_WARP;
        self.hue = DEFAULT_HUE;
        self.zoom = DEFAULT_ZOOM;
        self.glow = DEFAULT_GLOW;
        self.flash = DEFAULT_FLASH;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "warp" => self.warp = value,
            "field_speed" => self.field_speed = value,
            "fold_speed" => self.fold_speed = value,
            "hue" => self.hue = value,
            "zoom" => self.zoom = value,
            "glow" => self.glow = value,
            "flash" => self.flash = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Fully parameter-driven; the analysis reaches this scene only through
        // the preset expressions bound to its parameters. The one thing that
        // happens here is the phase integration, which runs after `set_param`
        // so it uses this frame's rates (ADR-0132).
        self.phases.step(self.fold_speed, self.field_speed, self.dt);
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // Upload the active palette LUTs (A + B) if a preset switch changed them
        // (off the hot path — once per switch, not per frame).
        if self.palette_dirty {
            palette::write_lut(queue, &self.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &self.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }

        let params = Params {
            a: [self.time, aspect.max(0.1), self.warp, self.hue],
            b: [self.zoom, self.glow, self.flash, self.color_span],
            c: [self.pan_x, self.pan_y, self.color_center, self.saturation],
            d: [
                self.palette_mix,
                self.occlude,
                palette::band_steps(self.palette_steps),
                palette::band_contour(self.palette_contour),
            ],
            e: [self.phases.fold, self.phases.field, 0.0, 0.0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&params));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fragment-field-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018); this fullscreen
                    // field is opaque, so it covers the backdrop as before.
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
        pass.set_bind_group(1, &self.lut_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property ADR-0132 exists for, and the one `warp_mesh`'s `time *
    /// wspeed` fails: a rate change moves the phase by `rate * dt` **whatever
    /// the elapsed scene time**. Under a multiply the same change at t = 100 s
    /// would move the picture fifty seconds in one frame — a teleport, on a lane
    /// whose whole method is binding parameters to audio.
    #[test]
    fn a_rate_change_advances_the_phase_by_rate_times_dt_at_any_elapsed_time() {
        let dt = 1.0 / 60.0;
        let mut phases = Phases::default();
        // Run a long way out, so a multiply-by-elapsed-time would be obvious.
        for _ in 0..6_000 {
            phases.step(1.0, 1.0, dt);
        }
        let elapsed = phases.fold;
        assert!(
            elapsed > 99.0,
            "the fixture must be far from t = 0: {elapsed}"
        );

        // Now the preset's binding moves, as an audio-bound rate does.
        let before = phases.fold;
        phases.step(1.5, 0.25, dt);
        let fold_step = phases.fold - before;
        assert!(
            (fold_step - 1.5 * dt).abs() < 1e-4,
            "the fold advanced {fold_step}, not {} — a phase that scales with \
             elapsed time is the defect",
            1.5 * dt
        );

        // And each accumulator carries its own rate: the pair is not welded.
        let field_before = phases.field;
        phases.step(1.5, 0.25, dt);
        // Tolerance is above the f32 ulp at this magnitude (~7.6e-6 near 100),
        // which is the accumulation cost ADR-0132 accepts.
        assert!(
            (phases.field - field_before - 0.25 * dt).abs() < 1e-4,
            "the field phase must follow field_speed alone: moved {}",
            phases.field - field_before
        );
    }

    /// At a constant rate the integrated phase equals `rate * t` — which is why
    /// every shipped preset, all of which leave both parameters at their `1.0`
    /// default, renders unchanged. Asserted rather than assumed, because it is
    /// what makes moving no golden a property rather than an observation.
    #[test]
    fn a_constant_rate_integrates_to_rate_times_elapsed_time() {
        let dt = 1.0 / 60.0;
        for rate in [1.0f32, 0.4, 2.5] {
            let mut phases = Phases::default();
            let mut clock = 0.0f32;
            for _ in 0..600 {
                phases.step(rate, rate, dt);
                clock += dt;
            }
            assert!(
                (phases.fold - rate * clock).abs() < 1e-3,
                "rate {rate}: integrated {} against {}",
                phases.fold,
                rate * clock
            );
        }
    }

    /// The default is exactly `1.0` on both, so the phase is bit-identical to
    /// the clock the three shader literals read — the capture path
    /// accumulates `time` the same way, one `FALLBACK_DT` per frame.
    #[test]
    fn the_default_rates_make_the_phase_the_clock() {
        assert_eq!(DEFAULT_FIELD_SPEED, 1.0);
        assert_eq!(DEFAULT_FOLD_SPEED, 1.0);

        let dt = crate::render::scenes::FALLBACK_DT;
        let mut phases = Phases::default();
        let mut clock = 0.0f32;
        for _ in 0..240 {
            phases.step(DEFAULT_FOLD_SPEED, DEFAULT_FIELD_SPEED, dt);
            clock += dt;
        }
        assert_eq!(
            phases.fold, clock,
            "at rate 1.0 the accumulation must be bit-identical to the clock's"
        );
        assert_eq!(phases.field, clock);
    }
}
