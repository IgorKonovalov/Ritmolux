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

/// `fb_zoom` default — a factor of `1.0` **per second**, i.e. no scaling however
/// long the frame is.
pub(crate) const DEFAULT_FB_ZOOM: f32 = 1.0;
/// `fb_rotate` / `fb_dx` / `fb_dy` / `fb_warp` default — zero rad/s, zero units/s.
pub(crate) const DEFAULT_FB_RATE: f32 = 0.0;
/// `fb_center_x` / `fb_center_y` default — the middle of the frame, in uv.
pub(crate) const DEFAULT_FB_CENTER: f32 = 0.5;

/// **The** `fb_*` vocabulary, in one place (ADR-0048).
///
/// Both sinks declare these names in their own `PARAMS` — the trails stage as
/// part of the composite's global vocabulary, the attractor as part of its
/// system's — because each has a `set_param` match that must be checkable against
/// its own list (`core/tests/preset.rs`'s drift guard reads the source text). This
/// const is what those two lists are checked *against*, so "one vocabulary" is a
/// test rather than a comment.
pub(crate) const PARAMS: &[&str] = &[
    "fb_zoom",
    "fb_rotate",
    "fb_dx",
    "fb_dy",
    "fb_center_x",
    "fb_center_y",
    "fb_warp",
];

/// The `fb_*` transform as a preset states it: rates per second, centre in uv.
///
/// Shared by **both** accumulation sinks (ADR-0048), which is the point: the
/// trails stage and the attractor's internal trail resolve identical params
/// through identical arithmetic into identical uniform bytes, and then transform
/// their own buffer with the same shader snippet ([`TRANSFORM_WGSL`]). A second
/// copy of this maths is how the two would drift apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    /// Scale factor **per second**, applied as `zoom^dt`.
    pub(crate) zoom: f32,
    /// Radians per second.
    pub(crate) rotate: f32,
    /// Translation per second, in units of the target's **height** (so a `1.0`
    /// crosses the frame vertically in a second, and the same value crosses it
    /// horizontally in `aspect` seconds — one isotropic vocabulary).
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    /// The fixed point everything above turns about, in uv.
    pub(crate) centre_x: f32,
    pub(crate) centre_y: f32,
    /// `fb_warp` — the strength of whichever [`Warp`] the preset selected, per
    /// second like every other rate here. Inert at `0`, and inert at any value
    /// when the selected kind is [`Warp::None`].
    pub(crate) warp: f32,
}

impl Transform {
    /// Every `fb_*` at its default: the past sits still, exactly as it did before
    /// ADR-0048.
    pub(crate) const IDENTITY: Self = Self {
        zoom: DEFAULT_FB_ZOOM,
        rotate: DEFAULT_FB_RATE,
        dx: DEFAULT_FB_RATE,
        dy: DEFAULT_FB_RATE,
        centre_x: DEFAULT_FB_CENTER,
        centre_y: DEFAULT_FB_CENTER,
        warp: DEFAULT_FB_RATE,
    };

    /// Apply one `fb_*` name, returning whether this type owned it. A sink's
    /// `set_param` delegates here rather than matching the seven names itself, so
    /// the two sinks cannot disagree about what `fb_dx` means.
    pub(crate) fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "fb_zoom" => self.zoom = value,
            "fb_rotate" => self.rotate = value,
            "fb_dx" => self.dx = value,
            "fb_dy" => self.dy = value,
            "fb_center_x" => self.centre_x = value,
            "fb_center_y" => self.centre_y = value,
            "fb_warp" => self.warp = value,
            _ => return false,
        }
        true
    }

    /// Whether this frame's transform moves nothing — the flag the shader
    /// `select`s on, and the whole basis of ADR-0048's byte-identity claim.
    ///
    /// `kind` is the preset's `[feedback] warp`: `fb_warp` alone moves nothing
    /// when no kind is selected, and no kind moves anything at zero strength, so
    /// both have to be off their defaults before the warp is live.
    ///
    /// The centre is deliberately **not** tested: it is the fixed point, so with
    /// no scale, rotation, translation or warp it names a point nothing moves
    /// about. A non-finite term counts as identity for the same reason a decay
    /// factor is clamped — a `NaN` uv would sample garbage for the rest of the run.
    pub(crate) fn is_identity(&self, kind: Warp) -> bool {
        let finite = self.zoom.is_finite()
            && self.rotate.is_finite()
            && self.dx.is_finite()
            && self.dy.is_finite()
            && self.centre_x.is_finite()
            && self.centre_y.is_finite()
            && self.warp.is_finite();
        !finite
            || (self.zoom == DEFAULT_FB_ZOOM
                && self.rotate == DEFAULT_FB_RATE
                && self.dx == DEFAULT_FB_RATE
                && self.dy == DEFAULT_FB_RATE
                && (kind == Warp::None || self.warp == DEFAULT_FB_RATE))
    }

    /// Pack `dt` seconds of this transform for [`TRANSFORM_WGSL`], about `aspect`
    /// — **the render target's**, never an internal grid's (ADR-0037).
    ///
    /// Returns the three `vec4`s the snippet reads, in its order: `xf`, `tr`, `wp`.
    pub(crate) fn pack(&self, dt: f32, aspect: f32, kind: Warp) -> [[f32; 4]; 3] {
        let theta = self.rotate * dt;
        let (sin, cos) = theta.sin_cos();
        // `zoom^dt`: a factor per second, so two half-length frames scale the
        // past by exactly what one full-length frame would.
        let scale = self.zoom.powf(dt);
        // Guard the reciprocal: a preset may sweep `fb_zoom` through 0 (a
        // `[smoothing]` ease is continuous), and `1/0` is `inf` — every pixel
        // would then sample the same texel forever.
        let inv_scale = if scale.is_finite() && scale.abs() > f32::MIN_POSITIVE {
            1.0 / scale
        } else {
            1.0
        };
        [
            [cos, sin, inv_scale, aspect],
            [self.dx * dt, self.dy * dt, self.centre_x, self.centre_y],
            // Strength per second like the rest, so a warp's advance per frame is
            // the same wall-clock gesture at any refresh. Zeroed when no kind is
            // selected, so the shader's `kind` branch is the only thing that has
            // to agree with the preset.
            [
                kind.code(),
                if kind == Warp::None {
                    0.0
                } else {
                    self.warp * dt
                },
                0.0,
                0.0,
            ],
        ]
    }
}

/// **The** transform, as WGSL — concatenated into the feedback body of *both*
/// accumulation sinks (ADR-0048).
///
/// Written as free functions over explicit `vec4` arguments rather than against a
/// named uniform, because the two sinks pack these terms into different uniform
/// structs (the trails stage's carries a decay factor and an occlude; the
/// attractor's carries a retention factor and an occlude). What they share is the
/// arithmetic, and this is it — the alternative, ADR-0048's own "the cost is a
/// second shader", would be two copies of a rotation that must agree forever.
///
/// The caller supplies `xf`, `tr` and `wp` exactly as [`Transform::pack`] returns
/// them, and is responsible for the `select` that keeps the identity path on the
/// literal sample uv.
pub(crate) const TRANSFORM_WGSL: &str = r#"
// The `[feedback] warp` roster, as the CPU writes it — keep in step with
// `feedback::Warp::code`.
const LMV_WARP_SWIRL:   f32 = 1.0;
const LMV_WARP_RIPPLE:  f32 = 2.0;
const LMV_WARP_FISHEYE: f32 = 3.0;

// Radius (in frame-heights) at which the swirl has faded to ~1/e of its centre
// strength. Just over half a frame-height, so the vortex is a whole-frame gesture
// that still leaves the corners nearly still.
const LMV_SWIRL_SIGMA: f32 = 0.35;
// Ripple spatial frequency, rad per frame-height: ~2.9 wave crests between the
// centre and the top edge.
const LMV_RIPPLE_FREQ: f32 = 18.0;

// The curated procedural warp, in the same centred isotropic space the affine
// works in and about the same `fb_center_*`. Displaces the SOURCE coordinate, so
// a positive strength moves the past the way the docs say.
//
// A `kind` selector rather than four pipelines: coexisting pipelines with matching
// bind-group layouts mis-render on the DX12 WARP software adapter (ADR-0058), and
// the branch here is uniform across the draw anyway.
fn lmv_warp_source(p: vec2<f32>, wp: vec4<f32>) -> vec2<f32> {
    let kind = wp.x;
    let k = wp.y;
    let r = length(p);
    if (kind == LMV_WARP_SWIRL) {
        // Rotate by an angle that falls off as a Gaussian in radius — smooth
        // everywhere, unlike a linear falloff's kink at the cutoff radius.
        let a = k * exp(-(r * r) / (2.0 * LMV_SWIRL_SIGMA * LMV_SWIRL_SIGMA));
        let c = cos(a);
        let s = sin(a);
        return vec2<f32>(p.x * c + p.y * s, p.y * c - p.x * s);
    }
    if (kind == LMV_WARP_RIPPLE) {
        // Radial displacement by a standing wave in r. The guarded divide keeps
        // the direction defined at the exact centre, where there is no direction.
        let dir = p / max(r, 1e-4);
        return p - dir * (k * sin(r * LMV_RIPPLE_FREQ));
    }
    if (kind == LMV_WARP_FISHEYE) {
        return p * (1.0 + k * r * r);
    }
    return p;
}

// Where the pixel at `uv` was one frame ago — the INVERSE of the motion the params
// name, because a destination pixel asks where its content came from.
//
// The centred coordinate is made isotropic by scaling x by `xf.w`, which is the
// RENDER TARGET's aspect and never the accumulation grid's (ADR-0037), so the
// rotation is a rotation and not a shear; the scale-back on the way out cancels it.
fn lmv_source_uv(uv: vec2<f32>, xf: vec4<f32>, tr: vec4<f32>, wp: vec4<f32>) -> vec2<f32> {
    let aspect = xf.w;
    let centre = tr.zw;
    var p = uv - centre;
    p.x = p.x * aspect;
    // Undo this frame's translation, then its rotation (by -theta: the transpose
    // of R(theta)), then its scale — and last the warp, which therefore rides on
    // top of the affine rather than being carried through it.
    p = p - tr.xy;
    p = vec2<f32>(p.x * xf.x + p.y * xf.y, p.y * xf.x - p.x * xf.y);
    p = p * xf.z;
    p = lmv_warp_source(p, wp);
    p.x = p.x / aspect;
    return p + centre;
}

// The transparent-border edge policy: `1.0` inside the accumulation, `0.0`
// outside it. Off-frame reads contribute NOTHING — clamping would re-deposit the
// border texel every frame until the edge became a permanent bar of colour.
fn lmv_inside(uv: vec2<f32>) -> f32 {
    return f32(all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)));
}
"#;

/// Two offscreen textures a feedback scene ping-pongs between. Held by named
/// fields (not a `[_; 2]`) so read/write selection needs no array indexing on
/// the hot path.
pub(crate) struct PingPongField {
    // Kept alive so the views stay valid. Read only by `read_texture`, which is
    // test-only — hence the underscores, which are what keep a shipped build
    // from calling these fields dead.
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
                // `COPY_SRC` is here for **observability** and costs nothing on
                // the backends we ship: it is what lets a probe read the field's
                // own levels back rather than inferring them from the composite,
                // which is where two plans' worth of tone defects hid (Plan 0109
                // Phase 4). No shipped path copies from these textures.
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
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

    /// The texture behind [`read_view`](Self::read_view) — what the next pass
    /// samples, and so what the last one wrote. For measurement: a probe copies
    /// it to a readback buffer and reads the field's own levels in the field's
    /// own units, instead of reading the composite and arguing backwards through
    /// `gamma`, `brightness` and the present remaps.
    #[cfg(test)]
    pub(crate) fn read_texture(&self) -> &wgpu::Texture {
        if self.reading_a {
            &self._tex_a
        } else {
            &self._tex_b
        }
    }

    /// Flip read and write — call once after each sub-step's render pass.
    pub(crate) fn swap(&mut self) {
        self.reading_a = !self.reading_a;
    }
}

#[cfg(test)]
mod tests {
    //! The **one vocabulary, two buffers** contract (ADR-0048). GPU-free: these are
    //! facts about the rosters and the arithmetic, not about pixels.
    #![allow(clippy::panic)]

    use super::{PARAMS, Transform, Warp};

    /// Both sinks declare **exactly** the shared `fb_*` vocabulary — no more, no
    /// less.
    ///
    /// `core/tests/preset.rs`'s drift guard cannot see these names: it reads
    /// `set_param`'s match arms out of the source text, and both sinks *delegate*
    /// the seven to [`Transform::set_param`] rather than matching them. That is the
    /// right factoring — one implementation of what `fb_dx` means — and this is
    /// what replaces the coverage it costs.
    #[test]
    fn both_sinks_declare_exactly_the_shared_fb_vocabulary() {
        let sinks: [(&str, &[&str]); 2] = [
            ("trails stage", crate::render::trails::PARAMS),
            ("attractor scene", crate::render::scenes::particles::PARAMS),
        ];
        for (label, declared) in sinks {
            for name in PARAMS {
                assert!(
                    declared.contains(name),
                    "the {label} does not declare `{name}`, so a preset binding it \
                     would only reach the other sink — see ADR-0048's routing \
                     contract"
                );
            }
            // ...and nothing `fb_`-shaped that the shared roster does not know
            // about, which would be a name only one sink answered.
            for name in declared.iter().filter(|n| n.starts_with("fb_")) {
                assert!(
                    PARAMS.contains(name),
                    "the {label} declares `{name}`, which is not in the shared \
                     `feedback::PARAMS` roster — either add it there (so BOTH \
                     sinks get it) or it does not belong in an `fb_` namespace"
                );
            }
        }
    }

    /// [`Transform::set_param`] answers exactly the roster — the other half of the
    /// guard above, since a declared name that the shared setter drops would be a
    /// param both sinks list and neither applies.
    #[test]
    fn the_shared_setter_answers_exactly_the_roster() {
        let mut t = Transform::IDENTITY;
        for name in PARAMS {
            assert!(
                t.set_param(name, 0.25),
                "`{name}` is in the roster but `Transform::set_param` drops it"
            );
        }
        assert!(
            !t.set_param("trails", 0.5),
            "`trails` belongs to the stage, not to the shared transform"
        );
        assert!(!t.set_param("fb_nonsense", 1.0));
    }

    /// The identity is the whole basis of ADR-0048's byte-identity claim, so it is
    /// asserted directly: every default moves nothing, and each term on its own
    /// moves something.
    #[test]
    fn the_identity_is_exactly_the_defaults() {
        assert!(Transform::IDENTITY.is_identity(Warp::None));

        for name in PARAMS {
            let mut t = Transform::IDENTITY;
            t.set_param(name, 0.75);
            // Two of the seven are still the identity on their own, and for
            // different reasons. `fb_center_*` names the fixed *point*, and with
            // nothing moving there is nothing for it to be the fixed point of.
            // `fb_warp` is a strength for a kind this call does not select.
            let inert_alone = name.starts_with("fb_center") || *name == "fb_warp";
            assert_eq!(
                t.is_identity(Warp::None),
                inert_alone,
                "with only `{name}` off its default, is_identity should be \
                 {inert_alone}"
            );
        }

        // `fb_warp` alone is inert — a strength with no kind selected — and comes
        // alive the moment the preset names one.
        let mut warped = Transform::IDENTITY;
        warped.set_param("fb_warp", 2.0);
        assert!(
            warped.is_identity(Warp::None),
            "a warp strength with no `[feedback] warp` kind selected moves nothing"
        );
        assert!(!warped.is_identity(Warp::Swirl));
        assert!(
            Transform::IDENTITY.is_identity(Warp::Swirl),
            "a selected kind at zero strength moves nothing either"
        );

        // A non-finite term reads as identity rather than poisoning the sample uv
        // for the rest of the run.
        let mut broken = Transform::IDENTITY;
        broken.set_param("fb_rotate", f32::NAN);
        assert!(broken.is_identity(Warp::None));
    }

    /// The rates are **per second** (ADR-0019): at the capture step the packed
    /// terms are exactly what one 1/60 s frame of the stated rate should be, and
    /// the identity packs to a literal identity.
    #[test]
    fn the_pack_is_per_second_and_aspect_correcting() {
        let dt = crate::render::scenes::FALLBACK_DT;
        let [xf, tr, wp] = Transform::IDENTITY.pack(dt, 1.6, Warp::None);
        assert_eq!(
            xf,
            [1.0, 0.0, 1.0, 1.6],
            "identity: no rotation, unit scale"
        );
        assert_eq!(tr, [0.0, 0.0, 0.5, 0.5], "identity: no shift, centred");
        assert_eq!(wp, [0.0; 4], "identity: no warp");

        // `fb_zoom` is a factor per second, so a 1/60 s frame takes its 60th root.
        let mut zoomed = Transform::IDENTITY;
        zoomed.set_param("fb_zoom", 2.0);
        let [xf, _, _] = zoomed.pack(dt, 1.0, Warp::None);
        let scale = 1.0 / xf[2];
        assert!(
            (scale.powf(60.0) - 2.0).abs() < 1e-3,
            "sixty frames of `fb_zoom = 2` must double the past, got {}",
            scale.powf(60.0)
        );

        // Two half-length frames compose to one full-length one — the property
        // that makes the look identical at 120 Hz and 60 Hz.
        let [half, _, _] = zoomed.pack(dt / 2.0, 1.0, Warp::None);
        let composed = (1.0 / half[2]) * (1.0 / half[2]);
        assert!((composed - scale).abs() < 1e-6);

        // `fb_zoom = 0` cannot produce an infinite reciprocal: a `[smoothing]`
        // ease sweeps continuously and would pass through it.
        let mut collapsed = Transform::IDENTITY;
        collapsed.set_param("fb_zoom", 0.0);
        let [xf, _, _] = collapsed.pack(dt, 1.0, Warp::None);
        assert!(xf[2].is_finite(), "a zero zoom must not pack an infinity");
    }
}
