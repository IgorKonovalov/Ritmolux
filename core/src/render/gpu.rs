//! Shared wgpu descriptor boilerplate: bind-group-layout entries, the
//! scene-seam blend state, the fullscreen-pass pipeline, the
//! fullscreen-triangle vertex preludes, and the fixed-timestep accumulator.
//!
//! Nothing here decides anything — it is the repetition every pass in
//! `render/` and `render/scenes/` would otherwise paste, in one home,
//! so a wgpu API change is one edit and a new stage starts from the
//! same shapes as the existing ones.
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
//! **There is no unflipped prelude**, and a ping-pong chain is not an exception
//! to that: "every pass uses this convention, so the flips cancel" is false, and
//! no arrangement of neighbouring passes makes it true — an unflipped read
//! samples row `((p.y+1)/2)*H` while the fragment writes the opposite row, so
//! the mirror is **complete within one pass**. It shipped in the attractor's
//! decay pass, whose target the draw pass writes in clip space, and every
//! attractor rendered as `figure ∪ mirror(figure)` for the life of the scene
//! (ADR-0070).
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
// Buffers
// ---------------------------------------------------------------------------

/// A `STORAGE | COPY_DST` buffer of `size` bytes, unmapped.
///
/// Read-only on the GPU side is a property of the **layout entry**, not of the
/// buffer, so this says nothing about a bind-group shape (ADR-0058) — see
/// [`uniform_buffer`].
pub(crate) fn storage_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A `UNIFORM | COPY_DST` buffer of `size` bytes, unmapped.
///
/// `size` is `usize` because every call site passes `size_of::<T>()` for the
/// uniform struct it writes; the cast to `u64` happens once, here.
///
/// This decides nothing a bind-group layout can see. A uniform's *shape* — its
/// visibility mask and its `min_binding_size` — is declared on the layout entry
/// ([`uniform`]), not on the buffer, so routing every uniform through one
/// constructor cannot make two layouts collide (ADR-0058).
pub(crate) fn uniform_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

// ---------------------------------------------------------------------------
// The single-colour-attachment render pass
// ---------------------------------------------------------------------------

/// Begin a render pass over one colour attachment: no resolve target, no depth,
/// no occlusion query, `store: Store`, and a timestamp-write pair taken from
/// whichever [`PassTimer`] is armed on this thread.
///
/// That is the shape of every pass this crate encodes, and `label`, `view` and
/// `load` are the only things any of them vary. The value is not the saved
/// lines — it is that a wgpu field addition to `RenderPassColorAttachment` or
/// `RenderPassDescriptor` (`depth_slice` and `multiview_mask` each arrived this
/// way) is one edit here instead of a compiler-driven sweep over forty sites
/// nobody reads.
///
/// `store: Store` is unconditional: a pass whose result is discarded is a pass
/// that should not have been encoded. Anything wanting a second colour target, a
/// depth attachment, or `StoreOp::Discard` spells its own descriptor — this
/// helper is not the place to grow a flag for it.
///
/// **`label` is the row a timing table reports under**, so it doubles as the
/// pass's identity: two passes sharing a label are summed, and a label built
/// per frame would be a new row per frame.
///
/// The returned pass borrows `encoder` and nothing else: wgpu 30 holds the
/// attachment by `Arc` internally, so `view` need only outlive the call.
pub(crate) fn color_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'encoder> {
    // Claimed before the descriptor is spelled, so the `&QuerySet` the
    // descriptor borrows is a handle on this stack frame rather than a borrow
    // out of the thread-local — which cannot outlive `with_borrow_mut`.
    let slot = claim_timestamp_slot(label);
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: slot.as_ref().map(|slot| wgpu::RenderPassTimestampWrites {
            query_set: &slot.query_set,
            beginning_of_pass_write_index: Some(slot.begin),
            end_of_pass_write_index: Some(slot.end),
        }),
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Begin a compute pass, timed the same way [`color_pass`] is.
///
/// The two compute dispatches in the engine — the attractor's step and its
/// decay seed — would otherwise be the only passes a timing table could not
/// see, and they are the heaviest ones on the scene that motivates measuring
/// at all.
pub(crate) fn compute_pass<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    label: &str,
) -> wgpu::ComputePass<'encoder> {
    let slot = claim_timestamp_slot(label);
    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: slot.as_ref().map(|slot| wgpu::ComputePassTimestampWrites {
            query_set: &slot.query_set,
            beginning_of_pass_write_index: Some(slot.begin),
            end_of_pass_write_index: Some(slot.end),
        }),
    })
}

// ---------------------------------------------------------------------------
// Per-pass GPU timing (ADR-0245's instrument)
// ---------------------------------------------------------------------------

/// How many passes one frame may time.
///
/// A rich-tier dissolve with both chains carrying all three stages and a
/// six-level bloom encodes well under half of this; past it a pass is simply
/// untimed rather than the frame being refused, because an instrument that can
/// stop a show is worse than one with a blind spot. Two queries a pass, so the
/// query set holds `2 * this` — inside wgpu's own 4096 ceiling with room.
const MAX_TIMED_PASSES: usize = 256;

/// Bytes one resolved timestamp query occupies. wgpu resolves each query as a
/// `u64` tick count.
const QUERY_BYTES: u64 = 8;

/// The query set and the index pair one pass writes into it.
///
/// Carries an **owned** query-set handle rather than a borrow, because the
/// timer it came from lives in a thread-local and `with_borrow_mut`'s closure
/// is the whole of that borrow's life — see [`claim_timestamp_slot`]. The
/// handle is refcounted, so the clone is an atomic increment.
struct TimestampSlot {
    query_set: wgpu::QuerySet,
    begin: u32,
    end: u32,
}

thread_local! {
    /// The timer this thread's next encoded pass writes into, if any.
    ///
    /// **Ambient rather than threaded through every call site**, and that is
    /// the design rather than an accident: a pass is opened from forty places,
    /// including three [`PostStage`](super::post::PostStage) implementations
    /// whose `resolve` signature is the composite's contract, so an explicit
    /// parameter would mean widening that contract to carry an instrument. The
    /// arm/disarm pair is owned by exactly one caller — the frame tap — and a
    /// timer is *moved* in and back out, so there is no path on which a frame
    /// writes into a query set another frame is resolving.
    static ARMED: std::cell::RefCell<Option<PassTimer>> = const { std::cell::RefCell::new(None) };
}

/// Take the next slot for `label` out of the armed timer, or `None` when no
/// timer is armed — which is every frame the window draws.
fn claim_timestamp_slot(label: &str) -> Option<TimestampSlot> {
    ARMED.with_borrow_mut(|armed| armed.as_mut().and_then(|timer| timer.claim(label)))
}

/// Whether a timer is armed on this thread. Read by the test that holds the
/// window path to encoding no timestamp writes at all.
#[cfg(test)]
pub(crate) fn timer_armed() -> bool {
    ARMED.with_borrow(Option::is_some)
}

#[cfg(test)]
thread_local! {
    /// How many passes this thread has attached timestamp writes to.
    ///
    /// Per-thread like [`ARMED`], so two tests running as threads of one
    /// process cannot read each other's count. Incremented only inside
    /// [`PassTimer::claim`], which only runs while a timer is armed — so "an
    /// untapped frame did not move it" is a claim about the descriptor every
    /// pass was actually given.
    static TIMESTAMP_WRITES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Passes this thread has attached timestamp writes to since it started.
#[cfg(test)]
pub(crate) fn timestamp_writes_issued() -> u64 {
    TIMESTAMP_WRITES.get()
}

/// One frame's GPU timestamp query set, its resolve and readback buffers, and
/// the labels of the passes written into it.
///
/// Built once per [`FrameTap`](super::capture::FrameTap) and reused for every
/// frame of a tapped run, so a long run allocates nothing here.
pub(crate) struct PassTimer {
    query_set: wgpu::QuerySet,
    /// `QUERY_RESOLVE | COPY_SRC` — `resolve_query_set`'s destination, which
    /// cannot also be `MAP_READ`.
    resolve: wgpu::Buffer,
    /// `COPY_DST | MAP_READ`, the CPU-visible copy of the above.
    readback: wgpu::Buffer,
    /// Nanoseconds one timestamp tick represents, read off the queue once.
    period_ns: f32,
    /// One label per timed pass, in write order. Reused between frames rather
    /// than rebuilt, so the steady state allocates no strings.
    labels: Vec<String>,
    /// Passes the frame being encoded has claimed a slot for.
    claimed: usize,
    /// Whether a map is outstanding on [`readback`](Self::readback), so a
    /// collection that finds no map does not unmap a buffer that was never
    /// mapped.
    mapped: bool,
}

impl PassTimer {
    /// Build a timer, or `None` when the device has no timestamp queries — the
    /// software rasterizers, and any driver that does not offer the feature.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let queries = MAX_TIMED_PASSES as u32 * 2;
        let bytes = u64::from(queries) * QUERY_BYTES;
        Some(Self {
            query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("rlx-pass-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: queries,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rlx-pass-timestamps-resolve"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rlx-pass-timestamps-readback"),
                size: bytes,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period_ns: queue.get_timestamp_period(),
            labels: Vec::with_capacity(MAX_TIMED_PASSES),
            claimed: 0,
            mapped: false,
        })
    }

    /// Hand out the next index pair, recording `label` against it. `None` once
    /// the frame has spent [`MAX_TIMED_PASSES`].
    fn claim(&mut self, label: &str) -> Option<TimestampSlot> {
        if self.claimed >= MAX_TIMED_PASSES {
            return None;
        }
        let at = self.claimed;
        self.claimed += 1;
        // Overwrite in place where a previous frame already put a label at this
        // position: the pass order is stable frame to frame for a given preset,
        // so the common case copies bytes into an existing `String` rather than
        // allocating one.
        match self.labels.get_mut(at) {
            Some(existing) if existing == label => {}
            Some(existing) => {
                existing.clear();
                existing.push_str(label);
            }
            None => self.labels.push(label.to_owned()),
        }
        #[cfg(test)]
        TIMESTAMP_WRITES.set(TIMESTAMP_WRITES.get() + 1);
        let begin = at as u32 * 2;
        Some(TimestampSlot {
            query_set: self.query_set.clone(),
            begin,
            end: begin + 1,
        })
    }

    /// Record the resolve and the copy to the readback buffer into `encoder`,
    /// after the frame's passes have been encoded into the same one.
    fn resolve_into(&self, encoder: &mut wgpu::CommandEncoder) {
        let queries = self.claimed as u32 * 2;
        if queries == 0 {
            return;
        }
        encoder.resolve_query_set(&self.query_set, 0..queries, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve,
            0,
            &self.readback,
            0,
            u64::from(queries) * QUERY_BYTES,
        );
    }
}

/// Arm `timer` for the passes encoded next on this thread. Returns the timer
/// that was armed before, which is `None` in every use this engine makes.
pub(crate) fn arm_pass_timer(timer: Option<PassTimer>) -> Option<PassTimer> {
    ARMED.with_borrow_mut(|armed| std::mem::replace(armed, timer.map(PassTimer::rearmed)))
}

/// Take the armed timer back and record its resolve into `encoder` — the other
/// half of [`arm_pass_timer`], and the one that must run before the frame is
/// submitted.
pub(crate) fn disarm_pass_timer(encoder: &mut wgpu::CommandEncoder) -> Option<PassTimer> {
    let timer = ARMED.with_borrow_mut(Option::take);
    if let Some(timer) = timer.as_ref() {
        timer.resolve_into(encoder);
    }
    timer
}

impl PassTimer {
    /// The timer with its per-frame claim count reset — what arming means.
    fn rearmed(mut self) -> Self {
        self.claimed = 0;
        self
    }

    /// Ask for the resolved timestamps, without waiting.
    ///
    /// Asked for on the same submission as the frame's own readback and taken
    /// on the same poll, so the timings and the pixels they describe arrive
    /// together and neither costs a wait of its own.
    pub(crate) fn map(&mut self) {
        if self.claimed == 0 {
            return;
        }
        let bytes = self.claimed as u64 * 2 * QUERY_BYTES;
        self.readback
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, |_| {});
        self.mapped = true;
    }

    /// Release a map that will never be read — the frame whose readback failed.
    /// Leaving it mapped would make the buffer unrecordable for the rest of the
    /// run.
    pub(crate) fn discard(&mut self) {
        if self.mapped {
            self.readback.unmap();
            self.mapped = false;
        }
    }

    /// Fold the mapped timestamps into `costs`, a no-op when the map did not
    /// land. A pass whose two ticks are out of order — which the timestamp
    /// counter's own wrap can produce — contributes zero rather than a nonsense
    /// figure.
    ///
    /// **Called before the timer is re-armed**, so `labels` and `claimed` still
    /// describe the frame these timestamps came from.
    pub(crate) fn collect(&mut self, costs: &mut super::capture::PassCosts) {
        if !self.mapped {
            return;
        }
        let bytes = self.claimed as u64 * 2 * QUERY_BYTES;
        let slice = self.readback.slice(..bytes);
        if let Ok(mapped) = slice.get_mapped_range() {
            for (at, pair) in mapped.chunks_exact(QUERY_BYTES as usize * 2).enumerate() {
                let Some(label) = self.labels.get(at) else {
                    break;
                };
                let (Some(begin), Some(end)) = (pair.get(..8), pair.get(8..16)) else {
                    break;
                };
                let begin = u64::from_le_bytes(begin.try_into().unwrap_or([0; 8]));
                let end = u64::from_le_bytes(end.try_into().unwrap_or([0; 8]));
                let ticks = end.saturating_sub(begin);
                costs.add(label, ticks as f64 * f64::from(self.period_ns) / 1.0e6);
            }
            drop(mapped);
        }
        self.readback.unmap();
        self.mapped = false;
        costs.close_frame();
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
// The fullscreen SDF scene's wgpu graph
// ---------------------------------------------------------------------------

/// The wgpu graph a fullscreen signed-distance scene draws through: its
/// pipeline, its uniform buffer, its palette LUT pair, an optional storage
/// buffer, and the one or two bind groups it binds.
///
/// `fragment_field`, `shape_field` and `shape_collage` each held those five as
/// separate fields and each spelled the same render tail. What they keep is
/// everything that makes them different scenes — the uniform struct they pack,
/// what goes in the storage buffer, and the WGSL — plus, crucially, the
/// **bind-group layouts**, which they still declare themselves.
///
/// # It cannot be handed a layout to build, and that is deliberate
///
/// ADR-0058 forbids two layouts that can be live in one frame from sharing a
/// shape without recorded evidence, and these three are live together routinely
/// (a layered preset, an A/B dissolve). Their layouts are *already* distinct on
/// purpose — a two-group split in one, a declared `min_binding_size` in the
/// next, a fragment-visible storage entry in the third — and each carries a
/// comment saying so. A constructor that built layouts from parameters would put
/// that distinctness behind an argument nobody reads.
///
/// It would also make those layouts **invisible to the guard**:
/// `no_two_layouts_share_a_shape_without_recorded_evidence` enumerates every
/// layout in `core/src` by scanning the source for `create_bind_group_layout`
/// with literal entries, and asserts the enumeration has not shrunk. A layout
/// built here from arguments would drop three rows off that list and weaken
/// every assertion made on it. So the scene builds its own layout and its own
/// bind groups, and hands the finished groups over.
pub(crate) struct FullscreenScene {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    /// The per-element array the collage reads; `None` for the two scenes that
    /// evaluate their field analytically.
    storage: Option<wgpu::Buffer>,
    luts: super::palette::LutPair,
    /// Bind group 0, always bound.
    group0: wgpu::BindGroup,
    /// Bind group 1 — the fragment field's LUT group, which it keeps separate to
    /// hold its pipeline layout off the kaleidoscope's shape. `None` for the two
    /// scenes that bind everything in one group.
    group1: Option<wgpu::BindGroup>,
}

/// The buffers and the LUT pair, before the scene has declared its layouts.
///
/// The two-step build exists because the bind groups need the buffers and the
/// pipeline needs the layouts: the scene takes these, spells its own layout and
/// bind groups against them, and hands both back to
/// [`finish`](FullscreenParts::finish).
pub(crate) struct FullscreenParts {
    uniforms: wgpu::Buffer,
    storage: Option<wgpu::Buffer>,
    luts: super::palette::LutPair,
}

impl FullscreenParts {
    /// The uniform buffer and the LUT pair, both labelled from `stem`.
    pub(crate) fn new(device: &wgpu::Device, stem: &str, uniform_bytes: usize) -> Self {
        Self {
            uniforms: uniform_buffer(device, &format!("{stem}-params"), uniform_bytes),
            storage: None,
            // Seeded with the default `spectrum`; the renderer calls
            // `Scene::set_palette` before the first frame and the first `render`
            // uploads it, so the textures are valid even if it never does.
            luts: super::palette::LutPair::new(device, stem),
        }
    }

    /// Hand over the storage buffer this scene's shader reads.
    ///
    /// The buffer is the **scene's** to create — its size and what goes in it are
    /// scene-specific, and its bind-group entry is spelled beside the layout that
    /// declares it — so this takes a finished one rather than a byte count. Pass
    /// it after building the bind group that binds it.
    pub(crate) fn with_storage(mut self, storage: wgpu::Buffer) -> Self {
        self.storage = Some(storage);
        self
    }

    /// The uniform buffer, for the scene's own bind-group entry.
    pub(crate) fn uniforms(&self) -> &wgpu::Buffer {
        &self.uniforms
    }

    /// The LUT pair, for [`bind_entries`](super::palette::LutPair::bind_entries).
    pub(crate) fn luts(&self) -> &super::palette::LutPair {
        &self.luts
    }

    /// Build the pipeline over the scene's layouts and take ownership of its
    /// bind groups.
    ///
    /// `layouts` is a slice for the same reason [`fullscreen_pipeline`] takes
    /// one, and its length must match how many groups are passed: two layouts
    /// with a `None` `group1` is a pipeline whose second group is never bound.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish(
        self,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layouts: &[&wgpu::BindGroupLayout],
        group0: wgpu::BindGroup,
        group1: Option<wgpu::BindGroup>,
        target_format: wgpu::TextureFormat,
        blend: wgpu::BlendState,
        stem: &str,
    ) -> FullscreenScene {
        debug_assert_eq!(
            layouts.len(),
            1 + usize::from(group1.is_some()),
            "a pipeline layout group with no bind group is never bound"
        );
        FullscreenScene {
            pipeline: fullscreen_pipeline(device, shader, layouts, target_format, blend, stem),
            uniforms: self.uniforms,
            storage: self.storage,
            luts: self.luts,
            group0,
            group1,
        }
    }
}

impl FullscreenScene {
    /// Hold `palette` for upload on the next
    /// [`flush_palette`](Self::flush_palette).
    pub(crate) fn set_palette(&mut self, palette: &super::palette::Palette) {
        self.luts.set(palette);
    }

    /// Upload the held palette if it changed. Call once at the top of `render`.
    pub(crate) fn flush_palette(&mut self, queue: &wgpu::Queue) {
        self.luts.flush(queue);
    }

    /// Write this frame's uniform struct.
    pub(crate) fn write_uniform<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, value: &T) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(value));
    }

    /// Write this frame's storage array. A no-op when the scene asked for no
    /// storage buffer — which is a caller bug, so it trips in debug builds.
    pub(crate) fn write_storage<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, values: &[T]) {
        debug_assert!(
            self.storage.is_some(),
            "write_storage on a scene built without a storage buffer"
        );
        if let Some(storage) = self.storage.as_ref() {
            queue.write_buffer(storage, 0, bytemuck::cast_slice(values));
        }
    }

    /// Encode the draw: one load-preserving pass, the pipeline, the bind groups,
    /// and the fullscreen triangle.
    pub(crate) fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let mut pass = color_pass(encoder, label, view, load);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group0, &[]);
        if let Some(group1) = self.group1.as_ref() {
            pass.set_bind_group(1, group1, &[]);
        }
        pass.draw(0..3, 0..1);
    }
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
/// framebuffer row. See the module docs: an unflipped variant mirrors every such
/// read, which is why there is not one (ADR-0070).
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
