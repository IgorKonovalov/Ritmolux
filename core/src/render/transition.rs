//! Cross-preset dissolves (Plan 0023, ADR-0024): the controller that drives a
//! preset switch as a MilkDrop-style dissolve instead of an instant cut, and the
//! two-input blend pass that mixes the two sides.
//!
//! # Where the blend sits
//!
//! `background -> scene -> PostChain -> [blend] -> ink -> surface`. The blend is
//! **outside** the [`PostChain`](super::post::PostChain) and outside the
//! [`PostStage`](super::post::PostStage) trait, per ADR-0032: it samples **two**
//! textures, which a one-input `PostStage::begin` cannot express, and widening the
//! trait for one implementor is the erosion ADR-0031 exists to prevent. It sits
//! *before* ink because ADR-0028 requires the tone remap to read the **blended**
//! frame — `mix(paper, ink, luminance)` is non-linear, so remapping each side and
//! blending the results would show a tone neither preset configures.
//!
//! Without a transition running the blend is absent entirely: no textures, no
//! pipeline, and the chain resolves straight into ink's input (or the surface) —
//! the unchanged frame path, which is why a dissolve costs nothing between
//! switches.
//!
//! # The two sides, and why they are asymmetric
//!
//! The **outgoing** side is a *snapshot*: one ordinary composite of the outgoing
//! preset, captured into a texture on the dissolve's opening frame and then held
//! still. The **incoming** side composites live every frame. That asymmetry is the
//! floor-safe default (ADR-0024): two live composites would double the GPU cost of
//! every switch, and it is simply impossible when both presets name the same
//! `SystemKind` and therefore share one mutable scene object.
//!
//! Because the snapshot is taken by rendering the outgoing preset one last time —
//! rather than by re-rendering it beside the incoming one — a dissolve costs
//! **one composite per frame throughout**, exactly like no dissolve at all. The
//! opening frame presents that snapshot unchanged, which is what `t = 0` means.
//!
//! # Determinism
//!
//! `t` advances purely on the frontend's injected `dt` (Plan 0014 / ADR-0013) — no
//! wall clock, no beat analysis — so a dissolve captured through the headless
//! harness is reproducible frame-for-frame (NFR §6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The blend encodes a pass on every frame of every dissolve.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::ink::InkParams;

/// The engine's default dissolve duration, in seconds (ADR-0024: policy is
/// engine-configured in code, not preset-declared). One second is MilkDrop's
/// rough feel — long enough to read as continuous, short enough that a switch
/// still feels like a switch.
pub(crate) const DEFAULT_DURATION_SECS: f32 = 1.0;

/// The shortest duration [`Transition::new`] will honor. A zero or negative
/// duration would divide by zero when advancing `t`; clamping here keeps the
/// controller total on any policy value rather than pushing the check to callers.
const MIN_DURATION_SECS: f32 = 1.0 / 1000.0;

/// How one dissolve mixes its two sides. A **small fixed library** (ADR-0024),
/// not an open registry: each is a variant of one shader, selected by the `kind`
/// slot of the blend uniform.
///
/// Every kind is exactly the outgoing frame at `t = 0` and exactly the incoming
/// one at `t = 1` — the property that lets a dissolve's opening frame present
/// through the blend before the live side has ever been rendered into, and that
/// keeps either endpoint from snapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionKind {
    /// A straight linear mix. The calm default.
    Crossfade,
    /// An additive flare that peaks mid-dissolve and returns — the "burn" the
    /// additive line/particle families read naturally, and the reason the blend
    /// samples both textures rather than alpha-compositing one over the other.
    AddBurn,
    /// A brightness-ordered reveal: the outgoing frame's darkest pixels turn over
    /// first, its highlights last, so the incoming preset seems to eat the old one
    /// out of its own shadows.
    LumaDissolve,
    /// A soft-edged diagonal boundary sweeping across the frame.
    Wipe,
}

impl TransitionKind {
    /// The library, in rotation order. `Crossfade` is first so the pinned-default
    /// policy and the rotation's first step agree.
    pub(crate) const LIBRARY: [TransitionKind; 4] = [
        TransitionKind::Crossfade,
        TransitionKind::AddBurn,
        TransitionKind::LumaDissolve,
        TransitionKind::Wipe,
    ];

    /// The kind for the `n`-th dissolve of a run — a **deterministic** rotation
    /// over [`LIBRARY`](Self::LIBRARY), never a random pick, so a captured show is
    /// reproducible (NFR §6).
    pub(crate) fn rotating(n: u32) -> Self {
        let index = (n as usize) % Self::LIBRARY.len();
        *Self::LIBRARY
            .get(index)
            .unwrap_or(&TransitionKind::Crossfade)
    }

    /// The `kind` slot the shader switches on.
    fn code(self) -> f32 {
        match self {
            TransitionKind::Crossfade => 0.0,
            TransitionKind::AddBurn => 1.0,
            TransitionKind::LumaDissolve => 2.0,
            TransitionKind::Wipe => 3.0,
        }
    }
}

/// The fidelity a dissolve runs its **outgoing** side at (ADR-0024).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The outgoing side is the frame captured at the dissolve's opening, held
    /// still. Always correct, always affordable — the floor-safe default, the
    /// same-scene answer, and the fallback the governor latches to.
    Freeze,
    /// The outgoing side keeps rendering live through its own composite, so both
    /// presets animate through the dissolve. The opportunistic upgrade.
    DualLive,
}

/// Whether a dissolve may take the dual-live upgrade — **pure** over the two
/// facts that decide it, so the governor is unit-testable with no GPU and no
/// clock.
///
/// Both must hold (ADR-0024):
///
/// - the two presets' scenes must not **share GPU state**, or one frame would
///   have to render one mutable object twice
///   ([`scenes::shares_resources`](super::scenes::shares_resources));
/// - the smoothed frame time must show **positive evidence of headroom**. A zero
///   `smoothed_frame_ms` means no samples — diagnostics collection is off, as it
///   is on every headless capture — and absence of evidence is not headroom, so
///   it declines. That is also what keeps a captured dissolve deterministic: with
///   no clock there is no clock-dependent mode.
pub(crate) fn dual_live_eligible(
    shares_resources: bool,
    smoothed_frame_ms: f32,
    budget_ms: f32,
) -> bool {
    if shares_resources {
        return false;
    }
    smoothed_frame_ms > 0.0 && smoothed_frame_ms <= budget_ms
}

/// Whether a **running** dual-live dissolve must give up and latch to the frozen
/// side — **pure**, the demotion counterpart of [`dual_live_eligible`].
///
/// Deliberately not the negation of eligibility: upgrading needs evidence of
/// headroom, demoting needs evidence of *overload*. A zero reading (no samples)
/// is neither, so it neither starts a dual-live dissolve nor kills one already
/// running. Once demoted a dissolve never upgrades again, so the mode cannot
/// flicker frame to frame.
pub(crate) fn budget_blown(smoothed_frame_ms: f32, budget_ms: f32) -> bool {
    smoothed_frame_ms > budget_ms
}

/// Where a running dissolve is in its two-stage lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The opening frame. The roster is still on the **outgoing** preset, so one
    /// ordinary composite captures exactly the frame an instant cut would have
    /// discarded. It is presented unchanged — `t = 0` is the outgoing look by
    /// definition — and the roster flips to the incoming preset at frame end.
    Capture,
    /// Every frame after. The roster is on the **incoming** preset, which
    /// composites live and is blended against the held snapshot by `t`.
    Dissolve,
}

/// One in-flight cross-preset dissolve — the **pure** controller half (no GPU, no
/// device, no wall clock), so its progression, its finalize index, and the
/// switch-mid-switch rule are unit-testable without a surface. The GPU half is
/// [`Blend`].
pub(crate) struct Transition {
    /// Roster index the dissolve started from — the preset the snapshot holds, and
    /// the one a dual-live dissolve keeps compositing live.
    from_index: usize,
    /// Roster index the dissolve lands on. Finalizing must leave the roster here
    /// exactly, whatever happened in between.
    to_index: usize,
    /// Progress in `[0, 1]`: 0 is exactly the outgoing look, 1 exactly the
    /// incoming one.
    t: f32,
    /// Total duration in seconds. Always at least [`MIN_DURATION_SECS`].
    dur: f32,
    /// How this dissolve mixes its two sides — chosen once, at the switch site.
    kind: TransitionKind,
    /// The fidelity of the outgoing side. Chosen at the switch site and only ever
    /// **downgraded** from there ([`latch_freeze`](Self::latch_freeze)).
    mode: Mode,
    stage: Stage,
    /// The outgoing preset's evaluated ink params, held at the capture frame so
    /// the one engine-wide ink pass can crossfade between the two sides
    /// (ADR-0032). `None` until that frame has run.
    outgoing_ink: Option<InkParams>,
}

impl Transition {
    /// Start a dissolve from `from_index` to `to_index` over `dur` seconds, mixed
    /// by `kind` at `mode` fidelity.
    pub(crate) fn new(
        from_index: usize,
        to_index: usize,
        dur: f32,
        kind: TransitionKind,
        mode: Mode,
    ) -> Self {
        let dur = if dur.is_finite() {
            dur.max(MIN_DURATION_SECS)
        } else {
            DEFAULT_DURATION_SECS
        };
        Self {
            from_index,
            to_index,
            t: 0.0,
            dur,
            kind,
            mode,
            stage: Stage::Capture,
            outgoing_ink: None,
        }
    }

    /// How this dissolve mixes its two sides.
    pub(crate) fn kind(&self) -> TransitionKind {
        self.kind
    }

    /// The roster index the dissolve started from — the side the snapshot holds,
    /// and the one a dual-live dissolve keeps compositing live.
    pub(crate) fn outgoing_index(&self) -> usize {
        self.from_index
    }

    /// Whether the outgoing side re-renders live this frame. Never true on the
    /// opening frame: that one *is* the outgoing preset's own composite.
    pub(crate) fn is_dual_live(&self) -> bool {
        self.mode == Mode::DualLive && self.stage == Stage::Dissolve
    }

    /// Override the fidelity the governor chose. **Test-only**: a headless capture
    /// has no meaningful frame-time clock — diagnostics are off and the readback
    /// blocks — so [`dual_live_eligible`] always answers `Freeze` there and the
    /// dual-live *render* path would otherwise be unreachable from a test.
    #[cfg(test)]
    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Give up on the live outgoing side for the **rest** of this dissolve. The
    /// frozen image is whatever the last live frame left in the outgoing target —
    /// so the fallback holds the picture still rather than jumping back to the
    /// frame the dissolve opened on. One-way: a demoted dissolve never upgrades,
    /// so the mode cannot flicker.
    pub(crate) fn latch_freeze(&mut self) {
        self.mode = Mode::Freeze;
    }

    /// Whether this frame is the opening one — the renderer must composite the
    /// still-active outgoing preset into the snapshot rather than the live target.
    pub(crate) fn needs_snapshot(&self) -> bool {
        self.stage == Stage::Capture
    }

    /// Current progress in `[0, 1]`, read **before** [`advance`](Self::advance).
    pub(crate) fn progress(&self) -> f32 {
        self.t
    }

    /// The roster index this dissolve lands on — where finalize must leave the
    /// roster, whatever arrived in between.
    pub(crate) fn incoming_index(&self) -> usize {
        self.to_index
    }

    /// The outgoing preset's held ink params, once the capture frame has run.
    pub(crate) fn outgoing_ink(&self) -> Option<&InkParams> {
        self.outgoing_ink.as_ref()
    }

    /// Advance the dissolve by `dt` real seconds, after the frame at the current
    /// [`progress`](Self::progress) has been encoded.
    ///
    /// Returns `Some(to_index)` on the capture frame — the renderer must flip the
    /// roster to that index and reconfigure its scene, so the next frame
    /// composites the incoming preset live. `outgoing_ink` is the ink params the
    /// capture frame evaluated for the outgoing preset; they are held for the rest
    /// of the dissolve.
    ///
    /// [`finished`](Self::finished) becomes true once `t` reaches 1.
    pub(crate) fn advance(&mut self, dt: f32, outgoing_ink: InkParams) -> Option<usize> {
        let step = if dt.is_finite() && dt > 0.0 {
            dt / self.dur
        } else {
            0.0
        };
        self.t = (self.t + step).min(1.0);
        if self.stage == Stage::Capture {
            self.stage = Stage::Dissolve;
            self.outgoing_ink = Some(outgoing_ink);
            return Some(self.to_index);
        }
        None
    }

    /// Whether the dissolve has reached its target and should be dropped, with the
    /// roster left on [`to_index`](Self::to_index).
    pub(crate) fn finished(&self) -> bool {
        self.t >= 1.0
    }
}

const SHADER: &str = r#"
struct Blend {
    // x = t in [0,1], y = TransitionKind::code(), rest unused
    p: vec4<f32>,
}

// Soft edge width for the two boundary kinds: how much of the sweep key a pixel
// takes to turn over. Wide enough to hide banding, narrow enough that the wipe
// reads as a boundary rather than a fade.
const LUMA_EDGE: f32 = 0.14;
const WIPE_EDGE: f32 = 0.07;
// Peak additive flare of the burn kind, at t = 0.5.
const BURN_PEAK: f32 = 0.35;

@group(0) @binding(0) var<uniform> u: Blend;
@group(0) @binding(1) var t_out: texture_2d<f32>;
@group(0) @binding(2) var t_in: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

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
    out.uv = vec2<f32>(0.5 * p.x + 0.5, 0.5 - 0.5 * p.y);
    return out;
}

// A boundary sweep: `key` in [0,1] orders the pixels, and the cut travels from
// just below 0 to just past 1 so the frame is *entirely* `a` at t = 0 and
// *entirely* `b` at t = 1 — no snap at either endpoint, whatever the key.
// Returns the per-pixel mix factor toward `b`.
fn sweep(key: f32, t: f32, edge: f32) -> f32 {
    let cut = mix(-edge, 1.0 + edge, t);
    return 1.0 - smoothstep(cut - edge, cut + edge, key);
}

fn luma(c: vec3<f32>) -> f32 {
    return clamp(dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Both sides are *sampled*, never alpha-composited: the line and particle
    // families draw additively, so laying one over the other at 1-t alpha would
    // produce wrong colors and could not express a non-crossfade kind (ADR-0024).
    let a = textureSample(t_out, samp, in.uv).rgb;
    let b = textureSample(t_in, samp, in.uv).rgb;
    let t = clamp(u.p.x, 0.0, 1.0);
    let kind = u32(u.p.y + 0.5);

    if (kind == 1u) {
        // Add/burn: the linear mix plus an additive flare that rises and falls,
        // zero at both endpoints so the ends stay exact. `4t(1-t)` peaks at 1.
        let flare = BURN_PEAK * 4.0 * t * (1.0 - t);
        return vec4<f32>(mix(a, b, t) + (a + b) * flare, 1.0);
    }
    if (kind == 2u) {
        // Luma dissolve: the outgoing frame's own brightness orders the reveal,
        // darkest first — the incoming preset eats the old one out of its shadows.
        return vec4<f32>(mix(a, b, sweep(luma(a), t, LUMA_EDGE)), 1.0);
    }
    if (kind == 3u) {
        // Wipe: a soft-edged diagonal boundary, top-left to bottom-right.
        let key = clamp((in.uv.x + in.uv.y) * 0.5, 0.0, 1.0);
        return vec4<f32>(mix(a, b, sweep(key, t, WIPE_EDGE)), 1.0);
    }
    // Crossfade (kind 0, and the fallback for any unknown code).
    return vec4<f32>(mix(a, b, t), 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlendUniform {
    p: [f32; 4],
}

/// The pipeline and everything sized independently of the surface — built once on
/// the first dissolve and kept for the process's life, so a switch never
/// recompiles a shader.
struct Pipeline {
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

/// The two surface-sized inputs and the bind group over them — dropped when a
/// dissolve finalizes, so the ~16 MB of full-frame textures a 1080p blend needs is
/// live only while one runs ("lightweight is a feature"). Split from [`Pipeline`]
/// along the axis that actually varies, like the attractor's resource split.
struct Targets {
    /// The **outgoing** side: the frozen composite captured on the opening frame.
    /// Kept alive so `snapshot_view` stays valid; not read after construction.
    _snapshot: wgpu::Texture,
    snapshot_view: wgpu::TextureView,
    /// The **incoming** side: the chain's destination on every dissolve frame.
    _live: wgpu::Texture,
    live_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// The two-input transition blend pass. Not a
/// [`PostStage`](super::post::PostStage) and never becomes one — see the module
/// docs.
pub(crate) struct Blend {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    pipeline: Option<Pipeline>,
    targets: Option<Targets>,
}

impl Blend {
    /// Store the device/format for a lazy build; no GPU resources yet, so a
    /// session that never switches a preset pays nothing.
    pub(crate) fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            pipeline: None,
            targets: None,
        }
    }

    /// Build (or rebuild, on a surface-size change) the two inputs and return
    /// their views: `(outgoing snapshot, incoming live)`. `None` only if the
    /// resources are absent, in which case the caller must fall back to an instant
    /// cut rather than a blend of undefined pixels.
    ///
    /// Both are surface-sized: the blend is a 1:1 per-pixel mix feeding a 1:1
    /// per-pixel ink remap, so a fixed internal grid would resample twice.
    fn ensure(&mut self, surface: (u32, u32)) -> Option<&Targets> {
        let (width, height) = (surface.0.max(1), surface.1.max(1));
        if self.pipeline.is_none() {
            self.pipeline = Some(Pipeline::build(&self.device, self.surface_format));
        }
        let pipeline = self.pipeline.as_ref()?;
        let stale = self
            .targets
            .as_ref()
            .is_none_or(|t| t.width != width || t.height != height);
        if stale {
            self.targets = Some(Targets::build(
                &self.device,
                self.surface_format,
                pipeline,
                width,
                height,
            ));
        }
        self.targets.as_ref()
    }

    /// The view the **outgoing** preset's composite is captured into on a
    /// dissolve's opening frame.
    pub(crate) fn snapshot_view(&mut self, surface: (u32, u32)) -> Option<wgpu::TextureView> {
        Some(self.ensure(surface)?.snapshot_view.clone())
    }

    /// The view the **incoming** preset's chain resolves into on every dissolve
    /// frame.
    pub(crate) fn live_view(&mut self, surface: (u32, u32)) -> Option<wgpu::TextureView> {
        Some(self.ensure(surface)?.live_view.clone())
    }

    /// Mix the two held inputs by `t` into `out` — ink's input when ink is active,
    /// the surface otherwise. Returns the draw calls encoded (1), or 0 if the
    /// resources are absent.
    ///
    /// At `t = 0` the result is the snapshot **exactly** — for every `kind`, which
    /// is what lets the opening frame present through this same pass before the
    /// live side has ever been rendered into.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        t: f32,
        kind: TransitionKind,
    ) -> u32 {
        let (Some(pipeline), Some(targets)) = (self.pipeline.as_ref(), self.targets.as_ref())
        else {
            return 0;
        };
        queue.write_buffer(
            &pipeline.uniform,
            0,
            bytemuck::bytes_of(&BlendUniform {
                p: [t.clamp(0.0, 1.0), kind.code(), 0.0, 0.0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blend-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &targets.bind_group, &[]);
        pass.draw(0..3, 0..1);

        1 // the blend pass
    }

    /// Drop the two full-frame inputs — on finalize, so they are live only while a
    /// dissolve runs, and on the capture scene-rebuild so a headless capture stays
    /// a pure function of its inputs (NFR §6). The pipeline survives: it is
    /// surface-independent and recompiling it per switch would be pure waste.
    pub(crate) fn release_targets(&mut self) {
        self.targets = None;
    }

    /// Drop **everything**, pipeline included — the capture rebuild path, matching
    /// the other stages' `reset_resources`.
    pub(crate) fn reset_resources(&mut self) {
        self.targets = None;
        self.pipeline = None;
    }
}

impl Pipeline {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blend-sampler"),
            // 1:1 same-size mix, so nearest is exact; clamp at the edges.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blend-uniform"),
            size: std::mem::size_of::<BlendUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blend-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blend-bind-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blend-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blend-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            uniform,
            sampler,
            bind_layout,
            pipeline,
        }
    }
}

impl Targets {
    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        pipeline: &Pipeline,
        width: u32,
        height: u32,
    ) -> Self {
        let side = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: surface_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let snapshot = side("blend-snapshot");
        let live = side("blend-live");
        let snapshot_view = snapshot.create_view(&wgpu::TextureViewDescriptor::default());
        let live_view = live.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blend-bind-group"),
            layout: &pipeline.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pipeline.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&live_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
            ],
        });

        Self {
            _snapshot: snapshot,
            snapshot_view,
            _live: live,
            live_view,
            bind_group,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    //! The controller contract, GPU-free: `t` is a pure function of the injected
    //! `dt` sequence, and finalize lands exactly on the requested index.

    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::{
        DEFAULT_DURATION_SECS, Mode, Transition, TransitionKind, budget_blown, dual_live_eligible,
    };
    use crate::render::ink::InkParams;

    /// A stand-in for the outgoing preset's evaluated ink params: the default
    /// remap (amount 0 = off), which is what an ink-less preset hands over.
    fn default_ink() -> InkParams {
        InkParams::default()
    }

    /// The opening frame is the capture frame, and advancing past it reports the
    /// index the renderer must flip the roster to.
    #[test]
    fn the_opening_frame_captures_then_flips_to_the_target() {
        let mut tr = Transition::new(
            2,
            5,
            DEFAULT_DURATION_SECS,
            TransitionKind::Crossfade,
            Mode::Freeze,
        );
        assert!(
            tr.needs_snapshot(),
            "the opening frame is the capture frame"
        );
        assert_eq!(tr.progress(), 0.0, "t = 0 is exactly the outgoing look");

        assert_eq!(
            tr.advance(1.0 / 60.0, default_ink()),
            Some(5),
            "the capture frame reports the index to flip the roster to"
        );
        assert!(!tr.needs_snapshot(), "only the opening frame captures");
        assert!(
            tr.outgoing_ink().is_some(),
            "the outgoing preset's ink params are held from the capture frame"
        );
    }

    /// `t` advances purely from the injected `dt` — no wall clock — so the same
    /// `dt` sequence always yields the same progress (NFR §6).
    #[test]
    fn progress_is_a_pure_function_of_the_injected_dt() {
        let run = |dts: &[f32]| {
            let mut tr = Transition::new(0, 1, 1.0, TransitionKind::Crossfade, Mode::Freeze);
            let mut seen = Vec::new();
            for &dt in dts {
                seen.push(tr.progress());
                tr.advance(dt, default_ink());
            }
            seen
        };
        let dts = [0.016, 0.020, 0.033, 0.016, 0.100];
        assert_eq!(run(&dts), run(&dts), "same dt sequence, same progress");

        // Half a second of a 1 s dissolve is halfway, whatever the frame rate:
        // 30 steps of 1/60 and 15 of 1/30 must agree.
        let mut fast = Transition::new(0, 1, 1.0, TransitionKind::Crossfade, Mode::Freeze);
        let mut slow = Transition::new(0, 1, 1.0, TransitionKind::Crossfade, Mode::Freeze);
        for _ in 0..30 {
            fast.advance(1.0 / 60.0, default_ink());
        }
        for _ in 0..15 {
            slow.advance(1.0 / 30.0, default_ink());
        }
        assert!(
            (fast.progress() - slow.progress()).abs() < 1e-5,
            "frame-rate independent: {} vs {}",
            fast.progress(),
            slow.progress()
        );
        assert!(
            (fast.progress() - 0.5).abs() < 1e-5,
            "half the duration is halfway: {}",
            fast.progress()
        );
    }

    /// A dissolve finishes at `t = 1` exactly — never past it — and its target
    /// index survives the whole run.
    #[test]
    fn a_dissolve_finishes_at_one_on_its_target() {
        let mut tr = Transition::new(3, 7, 0.25, TransitionKind::Crossfade, Mode::Freeze);
        let mut frames = 0;
        while !tr.finished() {
            tr.advance(1.0 / 60.0, default_ink());
            frames += 1;
            assert!(frames < 1000, "a 0.25 s dissolve must terminate");
        }
        assert_eq!(tr.progress(), 1.0, "t lands exactly on 1, never past it");
        assert_eq!(
            tr.incoming_index(),
            7,
            "finalize lands on the requested index"
        );
    }

    /// The engine policy's rotation is a **deterministic** walk over the whole
    /// library — never random, so a scripted sequence of switches reproduces the
    /// same kinds (NFR §6) — and it covers every kind before repeating, so a live
    /// show actually sees the library rather than one favourite.
    ///
    /// Pinning `TRANSITION_KIND` to `Some(kind)` in `render/mod.rs` bypasses this
    /// entirely, which is the one-line edit that changes every transition.
    #[test]
    fn the_kind_rotation_covers_the_library_in_order() {
        let library = TransitionKind::LIBRARY;
        let walk: Vec<TransitionKind> = (0..library.len() as u32)
            .map(TransitionKind::rotating)
            .collect();
        assert_eq!(
            walk,
            library.to_vec(),
            "the rotation is the library, in order"
        );

        for (i, kind) in library.iter().enumerate() {
            assert!(
                !library[..i].contains(kind),
                "the library must not repeat a kind: {kind:?} at {i}"
            );
        }

        // Deterministic and wrapping: the same n always gives the same kind, and
        // step n + len is step n.
        for n in 0..32u32 {
            assert_eq!(TransitionKind::rotating(n), TransitionKind::rotating(n));
            assert_eq!(
                TransitionKind::rotating(n),
                TransitionKind::rotating(n + library.len() as u32)
            );
        }
        // Even at the counter's wrap point, a kind comes back — never a panic.
        let _ = TransitionKind::rotating(u32::MAX);
    }

    /// The budget governor, over every case that decides a dissolve's fidelity
    /// (Plan 0023 Phase 4). This is where "a same-scene transition is verifiably
    /// freeze" is asserted: **shared scene resources veto dual-live outright**, no
    /// matter how much frame-time headroom there is.
    #[test]
    fn the_governor_only_upgrades_on_evidence_of_headroom() {
        const BUDGET: f32 = 18.0;

        // Shared resources: never dual-live. One mutable scene object cannot be
        // rendered twice in a frame, and the three line scenes share one renderer.
        for ms in [1.0, 8.0, BUDGET, 100.0] {
            assert!(
                !dual_live_eligible(true, ms, BUDGET),
                "a shared-resource pair must freeze even at {ms} ms"
            );
        }

        // Independent resources plus real headroom: upgrade.
        assert!(dual_live_eligible(false, 8.0, BUDGET));
        assert!(
            dual_live_eligible(false, BUDGET, BUDGET),
            "the budget is inclusive"
        );

        // Over budget: decline.
        assert!(!dual_live_eligible(false, BUDGET + 0.1, BUDGET));

        // No samples at all (diagnostics off — every headless capture) is not
        // evidence of headroom, so it declines. This is what keeps a captured
        // dissolve free of any clock-dependent choice.
        assert!(
            !dual_live_eligible(false, 0.0, BUDGET),
            "absence of frame-time evidence must not be read as headroom"
        );
    }

    /// Demotion is **not** the negation of eligibility: a zero reading neither
    /// starts a dual-live dissolve nor kills one already running, so a run with
    /// diagnostics off stays in whatever mode it began.
    #[test]
    fn the_governor_only_demotes_on_evidence_of_overload() {
        const BUDGET: f32 = 18.0;
        assert!(budget_blown(BUDGET + 0.1, BUDGET), "over budget demotes");
        assert!(!budget_blown(BUDGET, BUDGET), "exactly at budget holds");
        assert!(!budget_blown(1.0, BUDGET), "plenty of headroom holds");
        assert!(
            !budget_blown(0.0, BUDGET),
            "no samples is not evidence of overload"
        );
    }

    /// A dissolve's fidelity only ever goes **down**, and never on the opening
    /// frame — that frame is the outgoing preset's own composite, so there is
    /// nothing to re-render beside it.
    #[test]
    fn fidelity_latches_down_and_never_back_up() {
        let mut tr = Transition::new(0, 1, 1.0, TransitionKind::Crossfade, Mode::DualLive);
        assert!(
            !tr.is_dual_live(),
            "the opening frame is the outgoing composite itself, never a second one"
        );
        tr.advance(1.0 / 60.0, InkParams::default());
        assert!(tr.is_dual_live(), "the frames after it run both sides live");

        tr.latch_freeze();
        assert!(
            !tr.is_dual_live(),
            "a demoted dissolve stops rendering the outgoing side"
        );
        for _ in 0..30 {
            tr.advance(1.0 / 60.0, InkParams::default());
            assert!(!tr.is_dual_live(), "and never upgrades again");
        }
    }

    /// A degenerate duration cannot divide by zero or stall the dissolve.
    #[test]
    fn a_degenerate_duration_still_terminates() {
        for dur in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut tr = Transition::new(0, 1, dur, TransitionKind::Crossfade, Mode::Freeze);
            for _ in 0..200 {
                tr.advance(1.0 / 60.0, default_ink());
            }
            assert!(tr.finished(), "duration {dur} must still terminate");
            assert_eq!(tr.progress(), 1.0);
        }
    }

    /// A non-positive or non-finite `dt` holds the dissolve rather than jumping it
    /// backwards or to NaN — the frontend can inject either after a stall.
    #[test]
    fn a_degenerate_dt_holds_progress() {
        let mut tr = Transition::new(0, 1, 1.0, TransitionKind::Crossfade, Mode::Freeze);
        tr.advance(0.5, default_ink()); // past the capture frame
        let held = tr.progress();
        for dt in [0.0, -0.5, f32::NAN] {
            tr.advance(dt, default_ink());
            assert_eq!(tr.progress(), held, "dt {dt} must not move progress");
        }
    }
}
