//! Reusable ping-pong offscreen field for stateful feedback scenes (ADR-0012).
//!
//! Two same-format textures a simulation swaps each sub-step: the sim samples
//! the previous state (the *read* view) and writes the next (the *write* view),
//! then the field swaps so the fresh state becomes the next read. This is the
//! engine's first feedback path; the reaction-diffusion scene is its first user
//! and future warp/feedback variants reuse it (ADR-0002 named it a deferred
//! follow-up).
//!
//! **Composition, not engine machinery.** The field owns only the texture pair
//! and the read/write selector; the *scene* owns its sim/present pipelines and
//! the shader that steps the field. That keeps the `Scene` seam thin — ADR-0012
//! rejected an engine-managed multi-pass pipeline for exactly this reason.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). A feedback scene encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The curated procedural warp an accumulation resamples its past through, from
/// the `[feedback] warp` structural key (ADR-0048).
///
/// **Load-time, not bindable**, by `[curve] family`'s reasoning: a warp kind is a
/// shader path, not a quantity, and ADR-0021 already rejected bindable discrete
/// indexes for the flicker/hard-cut class of reasons. Its *strength* is the
/// ordinary bindable `fb_warp`, which is where a preset puts the audio.
///
/// The family is deliberately small (ADR-0048 Alternative A): an author-defined
/// per-pixel warp is a grammar-to-WGSL translator and a per-preset pipeline
/// compile, and it should be decided as that rather than smuggled in as a stage
/// option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Warp {
    /// No procedural warp — the affine alone. The default, and the identity.
    #[default]
    None,
    /// A vortex: the past rotates about the feedback centre by an angle that
    /// falls off with radius, so the middle spins faster than the rim.
    Swirl,
    /// Concentric standing waves in radius — the past breathes in rings.
    Ripple,
    /// A radial magnification that grows with radius: the periphery is drawn in
    /// (positive `fb_warp`) or pushed out (negative).
    Fisheye,
}

impl Warp {
    /// Every kind, in the order the error message lists them.
    pub const ALL: [Warp; 4] = [Warp::None, Warp::Swirl, Warp::Ripple, Warp::Fisheye];

    /// Parse a `[feedback] warp` value, or `None` if unknown (a load error — the
    /// preset is rejected rather than silently rendering unwarped).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Warp::None,
            "swirl" => Warp::Swirl,
            "ripple" => Warp::Ripple,
            "fisheye" => Warp::Fisheye,
            _ => return None,
        })
    }

    /// The canonical name — the exact string [`from_name`](Self::from_name)
    /// accepts. The two are inverses and the one place the mapping lives.
    pub fn as_str(self) -> &'static str {
        match self {
            Warp::None => "none",
            Warp::Swirl => "swirl",
            Warp::Ripple => "ripple",
            Warp::Fisheye => "fisheye",
        }
    }

    /// The selector this kind is written into a shader uniform as.
    ///
    /// One shader with a kind uniform, **not one pipeline per kind** (Plan 0046's
    /// own risk note): the DX12 WARP software adapter mis-renders coexisting
    /// pipelines whose bind-group layouts match, and four permutations of one
    /// stage is exactly the shape that bites.
    pub(crate) fn code(self) -> f32 {
        match self {
            Warp::None => 0.0,
            Warp::Swirl => 1.0,
            Warp::Ripple => 2.0,
            Warp::Fisheye => 3.0,
        }
    }
}

/// How this frame's light lands on the transformed past, from the
/// `[feedback] blend` structural key (ADR-0048).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Deposit {
    /// `accum = max(cur, prev * fade)` — the engine's only blend until ADR-0048,
    /// and still the default. Bounded by the source maximum.
    #[default]
    Max,
    /// `accum = cur + prev * fade` — echoes that **sum**. Its geometric series is
    /// bounded by `1 / (1 - fade)`, which under the `MAX_FADE = 0.98` ceiling is
    /// 50x, and it rolls off through ADR-0046's tonemap rather than clipping.
    /// Only viable at all because the composite runs in linear light above 1.0.
    Add,
}

/// A preset's `[feedback]` table: the two load-time choices about how an
/// accumulation reads its own past (ADR-0048).
///
/// **One vocabulary, two sinks.** This type and the `fb_*` params it accompanies
/// are consumed by *both* accumulation buffers — the engine trails stage and the
/// attractor scene's internal trail — and each transforms only its own. It lives
/// here, beside [`PingPongField`], rather than in either of them, because that is
/// what makes "one vocabulary" structural instead of a convention.
///
/// [`Default`] is the identity in both fields, so a preset with no `[feedback]`
/// table renders exactly what it rendered before the table existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeedbackConfig {
    /// Which procedural warp, if any, rides on top of the `fb_*` affine.
    pub warp: Warp,
    /// How this frame's light is deposited onto the faded past.
    pub blend: Deposit,
}

/// Two offscreen textures a feedback scene ping-pongs between. Held by named
/// fields (not a `[_; 2]`) so read/write selection needs no array indexing on
/// the hot path.
pub(crate) struct PingPongField {
    // Kept alive so the views stay valid; not read after construction.
    _tex_a: wgpu::Texture,
    _tex_b: wgpu::Texture,
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    /// `true`: read from A, write to B. `swap` flips it each sub-step.
    reading_a: bool,
}

impl PingPongField {
    /// The field's texel format. `Rgba16Float` is renderable and filterable on
    /// the wgpu targets we ship (DX12/Vulkan/Metal) — the R/G channels hold the
    /// two Gray-Scott species with the headroom the slow gradients need
    /// (ADR-0012 Risks: the `Rgba8Unorm` fallback would band).
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// Allocate the texture pair at a fixed internal `width`×`height` grid,
    /// decoupled from the surface size (ADR-0012: the simulation is
    /// resolution-independent). Contents are undefined until the scene's seed
    /// pass writes every texel before the first sub-step reads it.
    pub(crate) fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let make = |label: &str| {
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
                format: Self::FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let tex_a = make("lmv-ppf-a");
        let tex_b = make("lmv-ppf-b");
        let view_a = tex_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = tex_b.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _tex_a: tex_a,
            _tex_b: tex_b,
            view_a,
            view_b,
            reading_a: true,
        }
    }

    /// Texture A's view — a scene binds it once to build its A-read bind group.
    pub(crate) fn view_a(&self) -> &wgpu::TextureView {
        &self.view_a
    }

    /// Texture B's view — a scene binds it once to build its B-read bind group.
    pub(crate) fn view_b(&self) -> &wgpu::TextureView {
        &self.view_b
    }

    /// Whether A is the current read source (so the scene picks the matching
    /// pre-built bind group without rebuilding one each sub-step).
    pub(crate) fn reading_a(&self) -> bool {
        self.reading_a
    }

    /// The view the next sub-step (or the present pass) samples from.
    pub(crate) fn read_view(&self) -> &wgpu::TextureView {
        if self.reading_a {
            &self.view_a
        } else {
            &self.view_b
        }
    }

    /// The view the next sub-step renders into.
    pub(crate) fn write_view(&self) -> &wgpu::TextureView {
        if self.reading_a {
            &self.view_b
        } else {
            &self.view_a
        }
    }

    /// Flip read and write — call once after each sub-step's render pass.
    pub(crate) fn swap(&mut self) {
        self.reading_a = !self.reading_a;
    }
}
