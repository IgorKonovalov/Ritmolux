//! Shared wgpu descriptor boilerplate: bind-group-layout entries, the
//! fullscreen-pass pipeline, the fullscreen-triangle vertex preludes, and the
//! fixed-timestep accumulator.
//!
//! Nothing here decides anything — it is the repetition that had accumulated
//! four copies of the bind-entry helpers, two `fullscreen_pipeline`
//! implementations, and nine pasted vertex stages across `render/` and
//! `render/scenes/` (Plan 0031 Phase 5). One home, so a wgpu API change is one
//! edit and a new stage starts from the same shapes as the existing ones.
//!
//! # The three vertex preludes are not interchangeable
//!
//! Every fullscreen pass draws the same oversized triangle, but they disagree on
//! what the fragment stage receives, and the disagreement is **load-bearing**:
//!
//! - [`FULLSCREEN_VS_NDC`] hands the fragment stage raw clip-space coordinates,
//!   for shaders that evaluate a field analytically rather than sampling a
//!   texture (the backdrop, the fragment field).
//! - [`FULLSCREEN_VS_UV_FLIPPED`] hands it texture coordinates with **Y flipped**,
//!   for a pass sampling a texture some *other* pass rendered into with the
//!   ordinary render-target orientation (the composite stages).
//! - [`FULLSCREEN_VS_UV`] hands it texture coordinates **without** the flip, for
//!   a pass sampling a field written by another pass using this same convention,
//!   where the two flips would cancel (the reaction-diffusion and attractor
//!   ping-pong chains).
//!
//! Handing a shader the wrong one produces a vertically-mirrored effect. They are
//! three constants rather than one with a flag precisely so a call site has to
//! name which it means.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Construction-time code, but the pragma is the file-level
// convention for everything under render/.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// ---------------------------------------------------------------------------
// Bind-group layout entries
// ---------------------------------------------------------------------------

/// A fragment-visible sampled-texture layout entry.
///
/// `filterable` must match how the shader reads it: a pass using `textureLoad`
/// wants `false`, one using `textureSample` through a filtering sampler wants
/// `true`. Getting it wrong is a pipeline-creation validation error, not a
/// silent artifact.
pub(crate) fn texture(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A fragment-visible filtering-sampler layout entry.
pub(crate) fn sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// A uniform-buffer layout entry. `visibility` is explicit because the swarm and
/// attractor read their uniforms from the **vertex** stage while every fullscreen
/// pass reads them from the fragment stage.
pub(crate) fn uniform(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

// ---------------------------------------------------------------------------
// Fullscreen pass pipeline
// ---------------------------------------------------------------------------

/// A fullscreen-triangle render pipeline: `vs_main` + `fs_main` out of `shader`,
/// no vertex buffers, no depth.
///
/// Parameterized by `target_format` and `blend` because that is the whole
/// difference between the copies this replaced: the composite stages write
/// `REPLACE` to the surface format, the reaction-diffusion sim writes `REPLACE`
/// into its own field format, and the two accumulating scenes present
/// `PREMULTIPLIED_ALPHA_BLENDING` over the backdrop (ADR-0026).
///
/// `bind_layouts` is a slice because the fragment field splits its uniforms and
/// its palette LUT across **two** groups — deliberately, to keep its pipeline
/// layout structurally distinct from the kaleidoscope's and dodge a DX12 WARP
/// identical-layout mis-render (ADR-0021 / Plan 0020). Every other caller passes
/// one.
pub(crate) fn fullscreen_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    bind_layouts: &[&wgpu::BindGroupLayout],
    target_format: wgpu::TextureFormat,
    blend: wgpu::BlendState,
    label: &str,
) -> wgpu::RenderPipeline {
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> =
        bind_layouts.iter().copied().map(Some).collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}-pipeline-layout")),
        bind_group_layouts: &layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label}-pipeline")),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Compile a fullscreen-pass shader: one of the `FULLSCREEN_VS_*` preludes
/// followed by the caller's bindings and `fs_main`. Runs at pipeline
/// construction, so the one `format!` here is not a hot-path allocation.
pub(crate) fn fullscreen_shader(
    device: &wgpu::Device,
    label: &str,
    vertex_prelude: &str,
    body: &str,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{vertex_prelude}{body}").into()),
    })
}

// ---------------------------------------------------------------------------
// Fullscreen-triangle vertex preludes (see the module docs on why three)
// ---------------------------------------------------------------------------

/// Fullscreen triangle passing **clip-space** coordinates to the fragment stage
/// as `ndc`. For shaders that evaluate a field from position rather than sampling.
pub(crate) const FULLSCREEN_VS_NDC: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Single oversized triangle covers the viewport (no vertex buffer).
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pts[vi], 0.0, 1.0);
    out.ndc = pts[vi];
    return out;
}
"#;

/// Fullscreen triangle passing **texture** coordinates as `uv`, with **Y
/// flipped** — clip space is Y-up, a render target's texture space is Y-down, so
/// a pass sampling what another pass rendered needs this.
pub(crate) const FULLSCREEN_VS_UV_FLIPPED: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Fullscreen triangle; map clip space to [0,1] uv (y flipped for texture space).
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(0.5 * p.x + 0.5, 0.5 - 0.5 * p.y);
    return out;
}
"#;

/// Fullscreen triangle passing texture coordinates as `uv` **without** the Y
/// flip. For a ping-pong chain where every pass uses this convention, so the
/// flips would cancel: reading and writing agree, and the field never appears
/// inverted to itself.
pub(crate) const FULLSCREEN_VS_UV: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}
"#;

// ---------------------------------------------------------------------------
// Fixed-timestep accumulator
// ---------------------------------------------------------------------------

/// Drains a variable real `dt` into a whole number of fixed simulation steps,
/// carrying the remainder — so a simulation advances at the same rate on any
/// refresh (Plan 0014's injected `dt`, NFR §6) instead of one step per frame.
///
/// Extracted from the identical twelve lines the attractor and reaction-diffusion
/// scenes each carried (Plan 0031 Phase 5). **No clock**: `dt` is injected, so a
/// headless capture stepping a fixed `dt` sequence is reproducible.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FixedStep {
    /// Unspent real time, in seconds.
    accumulator: f32,
    /// The fixed simulation step, in seconds.
    step: f32,
    /// Ceiling on the steps one call may return, so a long stall (a breakpoint, a
    /// window drag) cannot turn into a hundreds-of-steps catch-up spike.
    max_substeps: u32,
}

impl FixedStep {
    /// A fresh accumulator over a fixed `step` (seconds) with a per-call ceiling.
    /// Both scenes pass compile-time constants.
    pub(crate) fn new(step: f32, max_substeps: u32) -> Self {
        Self {
            accumulator: 0.0,
            step,
            max_substeps,
        }
    }

    /// Add `dt` real seconds and return how many whole steps to run now, capped at
    /// `max_substeps`. The sub-step remainder **carries** to the next call, so
    /// simulated time tracks real time instead of drifting with the frame rate.
    ///
    /// The carry itself is capped at **one step**: after a clamped stall the
    /// backlog is dropped down to a single pending step, so the simulation slows
    /// rather than racing to catch up (ADR-0012). This is the extracted behavior
    /// of the two copies verbatim, `accumulator.min(step)` included — Phase 5 is a
    /// refactor, so the arithmetic must not move.
    ///
    /// Total on any `dt`: the loop is bounded by `max_substeps` regardless of the
    /// value, so an infinite or non-finite `dt` cannot hang a frame.
    pub(crate) fn advance(&mut self, dt: f32) -> u32 {
        self.accumulator += dt;
        let mut steps = 0u32;
        while self.accumulator >= self.step && steps < self.max_substeps {
            self.accumulator -= self.step;
            steps += 1;
        }
        self.accumulator = self.accumulator.min(self.step);
        steps
    }
}

#[cfg(test)]
mod tests {
    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic)]

    use super::*;

    /// The drain: `dt` shorter than a step runs nothing yet, and a long `dt` runs
    /// as many whole steps as it contains. Exact binary fractions throughout, so
    /// the assertions are about the arithmetic and not about float luck.
    #[test]
    fn fixed_step_drains_whole_steps() {
        let mut fs = FixedStep::new(0.25, 8);
        assert_eq!(fs.advance(0.125), 0, "half a step is not a step");
        assert_eq!(fs.advance(0.125), 1, "the two halves complete one step");
        assert_eq!(fs.advance(0.75), 3, "three whole steps in 0.75 s");
        assert_eq!(fs.advance(0.0), 0, "no time, no steps");
    }

    /// The remainder carries: leftovers accumulate into later steps, so simulated
    /// time tracks real time rather than running one step per frame.
    #[test]
    fn fixed_step_carries_the_remainder() {
        let mut fs = FixedStep::new(0.25, 64);
        // 0.125 s per call, so every *other* call completes a step — a per-frame
        // stepper would have run eight, and a truncating one zero.
        let mut total = 0;
        for _ in 0..8 {
            total += fs.advance(0.125);
        }
        assert_eq!(total, 4, "1.0 s of real time is four 0.25 s steps");

        // A dt that never divides the step evenly still tracks it: 96 frames of
        // 1/64 s is 1.5 s, which is six 0.25 s steps.
        let mut fs = FixedStep::new(0.25, 64);
        let mut total = 0;
        for _ in 0..96 {
            total += fs.advance(1.0 / 64.0);
        }
        assert_eq!(total, 6);
    }

    /// The ceiling bounds one call, and the surviving backlog is capped at a single
    /// step — a recovered stall must not be followed by frames of catch-up
    /// (ADR-0012). This pins the extracted `accumulator.min(step)` exactly.
    #[test]
    fn fixed_step_clamps_a_stall_and_caps_the_carry() {
        let mut fs = FixedStep::new(0.25, 4);
        assert_eq!(fs.advance(5.0), 4, "a 5 s stall is capped at max_substeps");
        assert_eq!(
            fs.advance(0.0),
            1,
            "the backlog is cut to one pending step, not the 16 the stall implied"
        );
        assert_eq!(fs.advance(0.0), 0, "and then it is spent");
        assert_eq!(fs.advance(0.25), 1, "normal stepping resumes");
    }

    /// Total on any `dt`: the loop is bounded by `max_substeps` whatever the input,
    /// so no value can hang a frame. `advance` runs on every frame of a live
    /// render, so this is the hot-path safety claim.
    #[test]
    fn fixed_step_is_total_on_degenerate_input() {
        let mut fs = FixedStep::new(0.25, 8);
        assert_eq!(
            fs.advance(f32::INFINITY),
            8,
            "an infinite dt is capped, not unbounded"
        );
        assert_eq!(fs.advance(f32::NAN), 0, "a NaN dt fires nothing");
        // NaN did not poison the accumulator: `min` keeps the non-NaN operand, so
        // the next call is still bounded rather than dead or runaway.
        assert!(fs.advance(0.0) <= 1);

        let mut fs = FixedStep::new(0.25, 8);
        assert_eq!(fs.advance(-1.0), 0, "a negative dt fires nothing");

        // A degenerate step size cannot spin: the ceiling still terminates the loop.
        let mut zero = FixedStep::new(0.0, 8);
        assert_eq!(
            zero.advance(1.0),
            8,
            "bounded by the ceiling, not by the step"
        );
        let mut nan_step = FixedStep::new(f32::NAN, 8);
        assert_eq!(
            nan_step.advance(1.0),
            0,
            "no comparison against NaN succeeds"
        );

        // A zero ceiling runs nothing and does not spin.
        let mut none = FixedStep::new(0.25, 0);
        assert_eq!(none.advance(10.0), 0);
    }
}
