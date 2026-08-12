//! Shared wgpu descriptor boilerplate: bind-group-layout entries, the
//! scene-seam blend state, the fullscreen-pass pipeline, the
//! fullscreen-triangle vertex preludes, and the fixed-timestep accumulator.
//!
//! Nothing here decides anything — it is the repetition that had accumulated
//! four copies of the bind-entry helpers, two `fullscreen_pipeline`
//! implementations, and nine pasted vertex stages across `render/` and
//! `render/scenes/` (Plan 0031 Phase 5). One home, so a wgpu API change is one
//! edit and a new stage starts from the same shapes as the existing ones.
//!
//! # The two vertex preludes are not interchangeable
//!
//! Every fullscreen pass draws the same oversized triangle, but they disagree on
//! what the fragment stage receives, and the disagreement is **load-bearing**:
//!
//! - [`FULLSCREEN_VS_NDC`] hands the fragment stage raw clip-space coordinates,
//!   for shaders that evaluate a field analytically rather than sampling a
//!   texture (the backdrop, the fragment field).
//! - [`FULLSCREEN_VS_UV_FLIPPED`] hands it texture coordinates with **Y flipped**,
//!   for any pass that samples a texture — a composite stage reading what another
//!   pass rendered, or a feedback pass re-reading the target it is writing.
//!
//! # A pass that samples the target it writes addresses it in framebuffer space
//!
//! Clip space is Y-up; `@builtin(position)` and texture space are Y-down. A
//! fullscreen fragment at clip `p.y` writes framebuffer row `(1 - (p.y+1)/2)*H`,
//! so the only `uv` that round-trips to that same row is the **flipped** one.
//! There used to be a third prelude here, `FULLSCREEN_VS_UV`, handing over
//! unflipped coordinates and justified as "for a ping-pong chain where every pass
//! uses this convention, so the flips would cancel". They do not cancel, and no
//! arrangement of neighbouring passes makes them: an unflipped read samples row
//! `((p.y+1)/2)*H` while the fragment writes the opposite row, so the mirror is
//! **complete within one pass**. It shipped in the attractor's decay pass, whose
//! target the draw pass writes in clip space, and every attractor rendered as
//! `figure ∪ mirror(figure)` for the life of the scene
//! ([ADR-0070](../../../docs/adrs/0070-a-feedback-pass-addresses-its-own-target-in-framebuffer-space.md)).
//!
//! The alternative to a round-tripping `uv` is to skip `uv` entirely and address
//! by `@builtin(position)` through `textureLoad`, which is exact — that is what
//! reaction-diffusion's Gray-Scott step does, and it is why RD was the one user of
//! the retired prelude that it never actually mirrored.
//!
//! Handing a shader the wrong one produces a vertically-mirrored effect. They are
//! two constants rather than one with a flag precisely so a call site has to
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
// The scene-seam blend state (Plan 0051, ADR-0056)
// ---------------------------------------------------------------------------

/// Additive light whose alpha is the fragment's **own coverage**, accumulating by
/// saturation rather than by summing. The blend state for every draw pipeline
/// that renders **directly into the post chain's input** (ADR-0056).
///
/// Colour is `One` / `One` — additive and unbounded, which is what the
/// linear-light composite exists for. Alpha is `One` / `OneMinusSrcAlpha`, i.e.
/// premultiplied OVER, so stacked quads accumulate coverage as
/// `a_out = a_src + a_dst * (1 - a_src)`: monotone, and bounded in `[0, 1]` **by
/// construction**.
///
/// # The invariant a shader using this must keep
///
/// **Emit `vec4(colour * g, g)`** — premultiplied colour, and an alpha equal to
/// the coverage that fragment actually has. A fragment that writes no light must
/// write no coverage. The chain's last stage resolves
/// `src.rgb + backdrop * (1 - src.a)` over the backdrop (ADR-0055), so a
/// constant `1.0` alpha discards the backdrop across the **whole quad** —
/// including everywhere the falloff is zero. Both call sites shipped exactly
/// that: the swarm's radial falloff over a square sprite left four hard-edged
/// `(0,0,0,1)` corners (~21 % of every sprite), and the line renderer's
/// across-the-stroke falloff left two dark rims. Plan 0051 fixed both.
///
/// The invariant is *"alpha equals this fragment's coverage"*, **not** "alpha is
/// never 1". A fullscreen field that covers every pixel correctly emits `1.0` —
/// `scenes/fragment_field.rs` does, and it does not draw through this state.
///
/// # Why the alpha factor is not `One` / `One`
///
/// Summing alpha additively is what produced Plan 0045 Phase 4b's defect one
/// stage downstream: source alpha exceeded 1, the blend's `1 - src.a` went
/// negative, and the frame *subtracted* the backdrop under its own brightest
/// regions. That needed an explicit clamp to repair. Here the saturation comes
/// free from the blend state, so an out-of-range alpha at this seam is
/// **unrepresentable** rather than clamped after the fact.
///
/// # What enforces it
///
/// Nothing structural — a third draw pipeline can emit a constant alpha exactly
/// as these two did. The guard is a **lit-backdrop capture test per draw seam**
/// (`bg_bright > 0`, asserting the backdrop arrives intact wherever the scene
/// wrote no light), one beside each of the two call sites: `scenes/swarm.rs` and
/// `scenes/lines/renderer.rs`. A new seam owes a third. At `bg_bright = 0` — the
/// setting every golden baseline runs — the defect is invisible, which is why it
/// shipped.
pub(crate) const ADDITIVE_LIGHT_SATURATING_COVERAGE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};

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
/// identical-layout mis-render (ADR-0058). Every other caller passes one.
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
// The shared integer hash (Plan 0082 Phase 1)
// ---------------------------------------------------------------------------

/// One round of the **lowbias32** bit-mixer and the top 24 bits as a unit
/// fraction, as WGSL text — prepended by any shader that needs deterministic
/// pseudo-randomness. Concatenated in like
/// [`feedback::TRANSFORM_WGSL`](super::feedback::TRANSFORM_WGSL), for the same
/// reason: two copies of a hash are two hashes as soon as one of them is edited.
///
/// # Why integer mixing and not `fract(sin(dot(p, k)) * 43758.5453)`
///
/// The trig idiom is the one every shader on the internet uses and it is
/// **disqualified in this repository** (ADR-0096): `sin`'s precision is
/// implementation-defined, so WARP and the hardware adapter would disagree on
/// essentially every pixel — which is indistinguishable from the ADR-0058
/// class of defect the whole golden suite exists to catch. Integer arithmetic on
/// `u32` is exact and identical on every adapter, so a hashed pass can be held to
/// **byte** equality across adapters rather than to a drift tolerance.
///
/// The attractor's step shader shipped this pair first (Plan 0073); the tonemap's
/// dither is the second caller, and this is the promotion Plan 0082 asked for
/// rather than a third copy. `hash_unit` in `scenes/particles/mod.rs` is the CPU
/// mirror of exactly these two functions and must move with them.
pub(crate) const HASH_WGSL: &str = r#"
// One round of a bit-mixer (the lowbias32 constants), so a small change in the
// input decorrelates the output.
fn mix32(v: u32) -> u32 {
    var h = v;
    h = h ^ (h >> 16u);
    h = h * 0x7FEB352Du;
    h = h ^ (h >> 15u);
    h = h * 0x846CA68Bu;
    h = h ^ (h >> 16u);
    return h;
}

// The top 24 bits as an unsigned fraction in [0, 1). It cannot reach 1.0.
fn unit01(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}
"#;

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
///
/// This is also the only correct choice for a **feedback** pass re-reading the
/// target it writes: the flip is what makes `uv` round-trip to the fragment's own
/// framebuffer row. See the module docs — the unflipped variant that used to sit
/// below this one mirrored every such read and has been retired (ADR-0070).
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
