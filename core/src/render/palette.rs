//! Shared palette system (ADR-0021, Plan 0020): one gradient baked once at load
//! into a 256-entry RGB lookup table (LUT) that every shader-colored scene
//! samples, replacing the per-scene hardcoded iq cosine `palette()`.
//!
//! A preset selects a built-in **named** palette (`spectrum`, `ember`, `ice`,
//! `mono`, `aurora`) or (Plan 0020 Phase 2) a list of custom **stops**. Named
//! palettes are themselves defined as built-in gradients — some generated from
//! the cosine model, some as stop lists — so named and custom share one baked-LUT
//! representation. The LUT is delivered to the GPU scenes (fragment field,
//! reaction-diffusion, attractor) as a 256×1 texture and sampled on the CPU by
//! the swarm; one bake, two consumers, no drift.
//!
//! **Baking is pure and off the hot path** — a function of the config only (no
//! clock, no randomness), run once on preset load — so it is deterministic
//! (NFR 6). [`Palette::sample`] is allocation-free and runs per particle per
//! frame (swarm), so this module carries the hot-path panic pragma.
//!
//! ## The colour space (ADR-0151)
//!
//! A `[palette]` stop in a `.toml` is **sRGB**, and [`srgb_to_linear`] decodes it
//! at the load boundary, so the LUT below holds linear light and a stop written
//! `#c81423` renders `#c81423`. The gradients defined *here* — the cosine and the
//! named stop lists — are engine values already in that space and go through no
//! decode; the cosine could not, being a generator rather than a triple.
//!
//! **The `spectrum` default *is* the cosine model exactly**, so a
//! preset that declares no `[palette]` is unaffected by this module —
//! the load-bearing no-regression guarantee, gated by a unit test
//! comparing sampled colors.
//!
//! ## Saturation (the single source of truth)
//!
//! `saturation` is a bindable modulation applied to the *sampled* color, not
//! baked into the LUT. It must be applied **identically** on the CPU (swarm) and
//! in every scene's WGSL, so the canonical definition lives here and each shader
//! mirrors it verbatim:
//!
//! ```text
//! luma = 0.299*r + 0.587*g + 0.114*b        (Rec. 601 luma)
//! out  = luma + (rgb - luma) * saturation    (1.0 = unchanged, 0.0 = grayscale)
//! ```
//!
//! `hue` is the other shared modulation: it offsets the LUT *sample coordinate*
//! (pre-sample), so it is applied where the coordinate is computed, not here.
//!
//! ## Banding (ADR-0078) — the other single source of truth
//!
//! `palette_steps` turns the smooth ramp into hard graphic bands by quantizing the
//! **palette coordinate** rather than the baked LUT: `t' = (floor(t·N) + 0.5)/N`
//! immediately before the sample. The bake above is untouched, which is the whole
//! point — the band count has to be *bindable to audio*, and quantizing during the
//! bake would cost a re-bake and a texture upload every frame, exactly the
//! per-frame work the bake exists to remove.
//!
//! [`band_coord`] is the canonical definition and every sample site mirrors it —
//! the CPU sites call it, the WGSL sites carry a commented verbatim copy, the way
//! `apply_saturation` mirrors [`desaturate`]. A test in this module asserts the
//! copies have not drifted.
//!
//! **`palette_contour` is scoped, and the scoping is a fact about the pipeline
//! rather than a policy.** A screen-constant contour width needs `fwidth`, which
//! exists only in a fragment shader — and the attractor and the swarm sample the
//! LUT once *per particle*, in the vertex stage and on the CPU respectively, where
//! a point sprite has a single palette coordinate and so there is no gradient
//! across it to contour. So **banding reaches every scene; contours reach the
//! continuous-field scenes** (the fragment field and reaction-diffusion).
//! `palette_contour` elsewhere is inert and nothing warns, because the param *is*
//! known — which is why `presets/README.md` says so beside it.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). `sample` runs per particle per frame in the swarm.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::f32::consts::TAU;

/// LUT resolution: 256 entries span the gradient's `t`, one texel per entry.
pub const LUT_SIZE: usize = 256;

/// One RGB entry — **linear light** in `[0, 1]`, used directly as color. An
/// authored `[palette]` stop is sRGB and is decoded into this space once at the
/// load boundary by [`srgb_to_linear`] (ADR-0151); the engine's own gradients
/// below are written in it directly.
pub type Rgb = [f32; 3];

/// Decode one sRGB-encoded channel in `[0, 1]` to linear light — the IEC
/// 61966-2-1 transfer function, exactly.
///
/// **This is the whole of what a `[palette]` stop goes through** (ADR-0151). The
/// LUT holds light and the display write encodes it again on the way to 8-bit, so
/// a stop consumed raw arrives lifted: `#c81423` renders `#dd4c64`, its green
/// channel nearly quadrupled. Applying the decode at the load boundary — where a
/// stop is validated, once per preset — leaves the LUT and every sample site
/// exactly as they were; the table is constant for its lifetime, so per-sample
/// decoding would buy nothing and cost the hot path.
///
/// Out-of-range input is clamped, so the function is total: the load boundary
/// already rejects a non-finite channel and clamps the array form, and this makes
/// the contract hold without depending on that.
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// [`srgb_to_linear`] per channel — the form the load boundary calls.
pub fn srgb_to_linear_rgb(rgb: Rgb) -> Rgb {
    let [r, g, b] = rgb;
    [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)]
}

/// Rec. 601 luma weights — the single definition of "brightness" the shared
/// `saturation` desaturates toward, mirrored verbatim in every scene's WGSL.
const LUMA: Rgb = [0.299, 0.587, 0.114];

/// How a built-in palette's gradient is generated. Named palettes map to one of
/// these; custom `stops` (Phase 2) reuse [`Gradient::Stops`], so named and custom
/// bake through the same path.
enum Gradient<'a> {
    /// The iq cosine model: `channel = a + b*cos(2π*(c*t + d))`.
    Cosine { a: Rgb, b: Rgb, c: Rgb, d: Rgb },
    /// A piecewise-linear gradient through `(at, color)` stops sorted by `at`.
    Stops(&'a [(f32, Rgb)]),
}

/// A built-in named palette. Extend as later work curates more; unknown names are
/// rejected at the load boundary (`schema.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedPalette {
    /// The exact current iq cosine — the **default**, so shipped presets are
    /// unchanged. `d = (0.10, 0.42, 0.62)` reproduces `fragment_field`/`swarm`.
    Spectrum,
    /// Warm embers: deep red through orange to pale gold.
    Ember,
    /// Cool ice: deep blue through cyan to near-white.
    Ice,
    /// Grayscale black → white.
    Mono,
    /// Aurora: deep green through teal to violet.
    Aurora,
}

impl NamedPalette {
    /// Parse a `[palette] name` string, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "spectrum" => NamedPalette::Spectrum,
            "ember" => NamedPalette::Ember,
            "ice" => NamedPalette::Ice,
            "mono" => NamedPalette::Mono,
            "aurora" => NamedPalette::Aurora,
            _ => return None,
        })
    }

    /// The gradient this named palette bakes from.
    fn gradient(self) -> Gradient<'static> {
        match self {
            // The exact fragment/swarm cosine (a=b=0.5, c=1, d as below).
            NamedPalette::Spectrum => Gradient::Cosine {
                a: [0.5, 0.5, 0.5],
                b: [0.5, 0.5, 0.5],
                c: [1.0, 1.0, 1.0],
                d: [0.10, 0.42, 0.62],
            },
            NamedPalette::Ember => Gradient::Stops(&[
                (0.0, [0.05, 0.01, 0.0]),
                (0.45, [0.6, 0.12, 0.02]),
                (0.75, [1.0, 0.42, 0.06]),
                (1.0, [1.0, 0.86, 0.52]),
            ]),
            NamedPalette::Ice => Gradient::Stops(&[
                (0.0, [0.0, 0.05, 0.18]),
                (0.5, [0.09, 0.42, 0.72]),
                (0.8, [0.4, 0.75, 0.92]),
                (1.0, [0.85, 0.95, 1.0]),
            ]),
            NamedPalette::Mono => {
                Gradient::Stops(&[(0.0, [0.0, 0.0, 0.0]), (1.0, [1.0, 1.0, 1.0])])
            }
            NamedPalette::Aurora => Gradient::Stops(&[
                (0.0, [0.0, 0.1, 0.06]),
                (0.4, [0.0, 0.8, 0.45]),
                (0.7, [0.0, 0.5, 0.7]),
                (1.0, [0.5, 0.1, 0.75]),
            ]),
        }
    }
}

/// A validated, ready-to-bake palette selection from a preset's `[palette]`
/// table — constructed at the load boundary (`schema.rs`), then trusted by
/// [`Palette::bake`] (validate-at-the-boundary).
#[derive(Debug, Clone)]
pub enum PaletteConfig {
    /// A built-in named palette.
    Named(NamedPalette),
    /// Custom gradient stops (`(at, color)`), pre-validated at the load boundary:
    /// sorted `at` in `0..=1`, ≥2 entries, parseable colors. Baked through the
    /// same stop path the named stop-list palettes use.
    Custom(Vec<(f32, Rgb)>),
}

impl PaletteConfig {
    /// The default when a preset declares no `[palette]` — the exact current
    /// cosine, so shipped presets are unchanged.
    pub fn default_spectrum() -> Self {
        PaletteConfig::Named(NamedPalette::Spectrum)
    }
}

/// An **A/B palette pair** baked into two 256-entry RGB LUTs (Plan 0020 Phase 4).
/// A preset declares palette A (`[palette]`) and, optionally, palette B
/// (`[palette_b]`); a bindable `palette_mix` (`0..1`) crossfades between them per
/// frame. With no `[palette_b]`, `lut_b == lut_a`, so `palette_mix` is a no-op and
/// a single-palette preset is unchanged. Sampled on the GPU (two 256×1 textures,
/// lerped in-shader) and on the CPU (via [`sample`](Palette::sample)) from the
/// same tables.
///
/// **`Clone`, deliberately not `Copy`** (Plan 0031 Phase 6): the struct is 6144
/// bytes (`[Rgb; 256]` twice), so `Copy` made any accidental by-value use a silent
/// 6 KB memcpy. A scene still holds its own baked copy for deferred upload — it
/// just has to say `.clone()` to get one.
#[derive(Clone)]
pub struct Palette {
    lut_a: [Rgb; LUT_SIZE],
    lut_b: [Rgb; LUT_SIZE],
}

impl Palette {
    /// Bake a single palette (A) into both LUTs, so `palette_mix` is a no-op.
    /// Pure; off the hot path (preset load only).
    pub fn bake(cfg: &PaletteConfig) -> Palette {
        let lut = bake_config(cfg);
        Palette {
            lut_a: lut,
            lut_b: lut,
        }
    }

    /// Bake an A/B pair for a `palette_mix` crossfade. Pure; off the hot path.
    pub fn bake_pair(a: &PaletteConfig, b: &PaletteConfig) -> Palette {
        Palette {
            lut_a: bake_config(a),
            lut_b: bake_config(b),
        }
    }

    /// The default palette (`spectrum`), used when a preset declares no
    /// `[palette]` table.
    pub fn default_spectrum() -> Palette {
        Palette::bake(&PaletteConfig::default_spectrum())
    }

    /// Sample the crossfaded palette at `t` with A/B `mix` (`0` = A, `1` = B),
    /// linearly interpolated with the same texel-center convention (and wrap) the
    /// GPU texture sampler uses, so the CPU (swarm) and GPU scenes color
    /// consistently. Allocation-free — the swarm calls this per particle per
    /// frame. `mix <= 0` returns palette A exactly (matching the GPU `mix` at 0),
    /// so `palette_mix = 0` is identical to palette A alone.
    pub fn sample(&self, t: f32, mix: f32) -> Rgb {
        let a = sample_lut(&self.lut_a, t);
        if mix <= 0.0 {
            return a;
        }
        let b = sample_lut(&self.lut_b, t);
        let m = mix.min(1.0);
        let [ar, ag, ab] = a;
        let [br, bg, bb] = b;
        [ar + (br - ar) * m, ag + (bg - ag) * m, ab + (bb - ab) * m]
    }

    /// Palette A's LUT as tight RGBA8 bytes for a 256×1 `Rgba8Unorm` texture
    /// upload. Alpha is opaque; the display surface is 8-bit, so 8-bit LUT storage
    /// adds no visible banding over the analytic cosine.
    pub fn lut_a_bytes(&self) -> [u8; LUT_SIZE * 4] {
        lut_to_bytes(&self.lut_a)
    }

    /// Palette B's LUT as tight RGBA8 bytes (the crossfade target texture).
    pub fn lut_b_bytes(&self) -> [u8; LUT_SIZE * 4] {
        lut_to_bytes(&self.lut_b)
    }
}

/// Sample one LUT at `t`, linearly interpolated with the texel-center convention
/// (and wrap) the GPU sampler uses. Shared by [`Palette::sample`] for both sides.
fn sample_lut(lut: &[Rgb; LUT_SIZE], t: f32) -> Rgb {
    // Texel centers sit at (i + 0.5)/N (matching `bake_gradient` and hardware
    // filtering), so map `t` to x = t*N - 0.5 and lerp the bracketing texels.
    let tw = t - t.floor(); // wrap to [0, 1)
    let x = tw * LUT_SIZE as f32 - 0.5;
    let i0 = x.floor().rem_euclid(LUT_SIZE as f32) as usize;
    let i1 = (i0 + 1) % LUT_SIZE;
    let frac = x - x.floor();
    let a = lut.get(i0).copied().unwrap_or([0.0; 3]);
    let b = lut.get(i1).copied().unwrap_or([0.0; 3]);
    let [ar, ag, ab] = a;
    let [br, bg, bb] = b;
    [
        ar + (br - ar) * frac,
        ag + (bg - ag) * frac,
        ab + (bb - ab) * frac,
    ]
}

/// One baked LUT as tight RGBA8 bytes (opaque alpha) for a 256×1 texture upload.
fn lut_to_bytes(lut: &[Rgb; LUT_SIZE]) -> [u8; LUT_SIZE * 4] {
    let mut out = [0u8; LUT_SIZE * 4];
    for (px, rgb) in out.chunks_exact_mut(4).zip(lut.iter()) {
        let [r, g, b] = *rgb;
        if let [pr, pg, pb, pa] = px {
            *pr = to_u8(r);
            *pg = to_u8(g);
            *pb = to_u8(b);
            *pa = 255;
        }
    }
    out
}

/// Bake a [`PaletteConfig`] into a single LUT (the named or custom gradient).
fn bake_config(cfg: &PaletteConfig) -> [Rgb; LUT_SIZE] {
    match cfg {
        PaletteConfig::Named(named) => bake_gradient(&named.gradient()),
        PaletteConfig::Custom(stops) => bake_gradient(&Gradient::Stops(stops)),
    }
}

/// The GPU LUT texture format. `Rgba8Unorm` is trivially filterable everywhere
/// and — since the display surface is itself 8-bit — adds no visible banding
/// over the analytic cosine (the no-regression concern), while needing no
/// half-float conversion on upload.
pub const LUT_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Create the shared 256×1 LUT texture a shader-colored scene binds and uploads
/// its baked palette into. Centralized here so the fragment, reaction-diffusion,
/// and attractor scenes stay byte-for-byte consistent (ADR-0021: one source both
/// the GPU and CPU sample). Seed it with [`write_lut`] before first use.
pub fn lut_texture(device: &wgpu::Device, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: LUT_SIZE as u32,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: LUT_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// The LUT sampler: linear filtering, **repeat** across `u` (so a hue rotation
/// past the gradient edge wraps like the cosine's periodic wheel) and clamp on
/// the single-row `v`.
pub fn lut_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rlx-lut-sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// Upload one baked LUT (`palette.lut_a_bytes()` / `lut_b_bytes()`) into its 256×1
/// texture. Off the hot path — called from a scene's deferred `set_palette`
/// upload (first frame after a preset switch).
pub fn write_lut(queue: &wgpu::Queue, texture: &wgpu::Texture, bytes: &[u8; LUT_SIZE * 4]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(LUT_SIZE as u32 * 4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: LUT_SIZE as u32,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

// ---------------------------------------------------------------------------
// The A/B LUT pair a shader-coloured scene owns
// ---------------------------------------------------------------------------

/// The two LUT textures, their views, the sampler, the baked palette awaiting
/// upload and the dirty flag — the set every shader-coloured scene owns to
/// render a `palette_mix` crossfade.
///
/// # The upload is deferred, and that is the invariant this type keeps
///
/// [`set`](LutPair::set) is called from `Scene::set_palette`, which has no
/// `Queue`: a preset switch bakes a palette on the CPU and the GPU upload has to
/// wait for the next frame. So `set` stores the palette and raises `dirty`, and
/// [`flush`](LutPair::flush) — called at the top of `render`, where a `Queue`
/// exists — uploads and clears it. Two `set`s between frames cost one upload;
/// a frame with no `set` costs none.
///
/// A **freshly constructed pair is dirty**, because its textures are empty. A
/// scene that builds its GPU resources lazily (or rebuilds them on a resize)
/// therefore gets the upload for free, but must re-`set` the palette it is
/// actually holding — [`new`](LutPair::new) can only seed the default.
///
/// # It owns resources, never a layout shape
///
/// [`bind_entries`](LutPair::bind_entries) takes all three binding numbers from
/// the caller and the caller's own `create_bind_group_layout` still spells the
/// entries. Nothing here can make two layouts share a shape, which is what
/// ADR-0058 forbids without recorded evidence — and the six scenes bind this
/// triple at genuinely different indices and in different orders.
pub struct LutPair {
    texture_a: wgpu::Texture,
    texture_b: wgpu::Texture,
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    sampler: wgpu::Sampler,
    palette: Palette,
    dirty: bool,
}

impl LutPair {
    /// Both textures, both views and the sampler, seeded with the default
    /// palette and **dirty** — the textures hold no bytes until the first
    /// [`flush`](LutPair::flush).
    ///
    /// `stem` names the pair: the textures are labelled `<stem>-lut-a` and
    /// `<stem>-lut-b`.
    pub fn new(device: &wgpu::Device, stem: &str) -> Self {
        let texture_a = lut_texture(device, &format!("{stem}-lut-a"));
        let texture_b = lut_texture(device, &format!("{stem}-lut-b"));
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture_a,
            texture_b,
            view_a,
            view_b,
            sampler: lut_sampler(device),
            palette: Palette::default_spectrum(),
            dirty: true,
        }
    }

    /// Hold `palette` for upload on the next [`flush`](LutPair::flush). A
    /// 6 KB array copy, off the hot path (preset switch or resource build).
    pub fn set(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        self.dirty = true;
    }

    /// Upload the held palette into both textures if anything has changed since
    /// the last call, and report whether it did.
    ///
    /// Called once per frame from `render`. The return value is what the unit
    /// test reads; the scenes ignore it.
    pub fn flush(&mut self, queue: &wgpu::Queue) -> bool {
        if !self.dirty {
            return false;
        }
        write_lut(queue, &self.texture_a, &self.palette.lut_a_bytes());
        write_lut(queue, &self.texture_b, &self.palette.lut_b_bytes());
        self.dirty = false;
        true
    }

    /// The palette A texture's view.
    pub fn view_a(&self) -> &wgpu::TextureView {
        &self.view_a
    }

    /// The palette B texture's view.
    pub fn view_b(&self) -> &wgpu::TextureView {
        &self.view_b
    }

    /// The shared LUT sampler.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// The three bind-group entries, at the binding numbers the caller names.
    ///
    /// **The array is ordered by LUT role — A, B, sampler — not by binding
    /// number**, because the callers disagree on both. `shape_field` and
    /// `shape_collage` bind the sampler at 0 and the textures at 1 and 2;
    /// `fragment_field` and `warp_mesh` bind A, B, sampler at 0, 1, 2; the
    /// attractor at 1, 2, 3 and reaction-diffusion at 3, 4, 5. Each entry
    /// carries its own `binding`, which is what wgpu matches against the layout,
    /// so spreading this array into an `entries` list in role order is correct at
    /// every one of them.
    pub fn bind_entries(
        &self,
        binding_a: u32,
        binding_b: u32,
        binding_sampler: u32,
    ) -> [wgpu::BindGroupEntry<'_>; 3] {
        [
            wgpu::BindGroupEntry {
                binding: binding_a,
                resource: wgpu::BindingResource::TextureView(&self.view_a),
            },
            wgpu::BindGroupEntry {
                binding: binding_b,
                resource: wgpu::BindingResource::TextureView(&self.view_b),
            },
            wgpu::BindGroupEntry {
                binding: binding_sampler,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
        ]
    }
}

// --- Banding (ADR-0078) -----------------------------------------------------

/// `palette_steps` default — 0, which is off: the smooth ramp every preset drew
/// before this existed.
pub const DEFAULT_PALETTE_STEPS: f32 = 0.0;
/// `palette_contour` default — 0, no contour.
pub const DEFAULT_PALETTE_CONTOUR: f32 = 0.0;
/// At or below this band count the banding is **off**, and off is the exact
/// identity rather than a degenerate case of the quantized path: one band would
/// snap the whole palette to `(0 + 0.5)/1`, a single flat colour.
pub const MIN_ACTIVE_STEPS: f32 = 1.0;
/// Ceiling on the band count. Past a few dozen bands over the range a preset's
/// `color_span` covers, the steps are narrower than the gradient's own 256-entry
/// resolution and the banding stops being visible as banding.
pub const MAX_PALETTE_STEPS: f32 = 64.0;

/// The band count the sample sites are handed: clamped into `[0, MAX]`, then
/// **rounded to an integer**, with a non-finite binding falling back to off.
///
/// This is `kaleidoscope.rs`'s `fold_order` treatment for `fold_order`'s reason,
/// on a different seam. `[smoothing]` and preset dissolves sweep a binding
/// *continuously* between two settings, and a fractional band count does not step
/// — it leaves every band boundary crawling across the field, one per frame, which
/// reads as shimmer rather than as a colour change. Rounding on the CPU keeps that
/// precondition on the CPU, where it is visible.
pub fn band_steps(steps: f32) -> f32 {
    if steps.is_finite() {
        steps.clamp(0.0, MAX_PALETTE_STEPS).round()
    } else {
        DEFAULT_PALETTE_STEPS
    }
}

/// Quantize a palette coordinate onto `steps` hard bands — **the canonical
/// definition** every LUT sample site in the engine mirrors (module docs).
///
/// `t' = (floor(t·N) + 0.5)/N` lands on each band's *centre*, so the colour a band
/// takes is the one the smooth ramp had in the middle of it rather than at its
/// edge. Below [`MIN_ACTIVE_STEPS`] (tested as `< 1.5`, since [`band_steps`] has
/// already rounded) the coordinate passes through **untouched** — the exact
/// identity, which is what keeps every shipped preset and every golden baseline
/// byte-identical.
///
/// Negative and above-1 coordinates are fine and are the common case: the LUT is
/// repeat-addressed, so a `color_span` above 1 wraps it, and `floor` keeps the
/// quantization aligned across every wrap.
pub fn band_coord(t: f32, steps: f32) -> f32 {
    if steps < 1.5 {
        return t;
    }
    ((t * steps).floor() + 0.5) / steps
}

/// The contour depth the fragment sites are handed: clamped to `[0, 1]`, with a
/// non-finite binding falling back to none.
///
/// The contour itself has no CPU definition to be canonical — it is drawn from
/// `fwidth`, which exists only in a fragment shader — so the WGSL is the
/// implementation and its two copies are what the drift test compares. This is the
/// part of it that *can* live on the CPU.
pub fn band_contour(contour: f32) -> f32 {
    if contour.is_finite() {
        contour.clamp(0.0, 1.0)
    } else {
        DEFAULT_PALETTE_CONTOUR
    }
}

/// Apply the shared `saturation` modulation to a sampled color — the canonical
/// CPU definition the WGSL mirrors (see the module docs). `1.0` is unchanged,
/// `0.0` is grayscale, `> 1.0` oversaturates.
pub fn desaturate(rgb: Rgb, saturation: f32) -> Rgb {
    let [r, g, b] = rgb;
    let [lr, lg, lb] = LUMA;
    let luma = r * lr + g * lg + b * lb;
    [
        luma + (r - luma) * saturation,
        luma + (g - luma) * saturation,
        luma + (b - luma) * saturation,
    ]
}

/// Bake a gradient into the 256-entry LUT. Entry `i` holds the color at the
/// texel center `t = (i + 0.5)/N`, so sampling the resulting texture (or
/// [`Palette::sample`]) at a coordinate `u` returns the gradient at `u` with
/// sub-texel accuracy.
fn bake_gradient(g: &Gradient<'_>) -> [Rgb; LUT_SIZE] {
    let mut lut = [[0.0f32; 3]; LUT_SIZE];
    for (i, slot) in lut.iter_mut().enumerate() {
        let t = (i as f32 + 0.5) / LUT_SIZE as f32;
        *slot = match g {
            Gradient::Cosine { a, b, c, d } => cosine_at(*a, *b, *c, *d, t),
            Gradient::Stops(stops) => stops_at(stops, t),
        };
    }
    lut
}

/// The iq cosine palette `a + b*cos(2π*(c*t + d))` per channel, clamped to
/// `[0, 1]`.
fn cosine_at(a: Rgb, b: Rgb, c: Rgb, d: Rgb, t: f32) -> Rgb {
    let [ar, ag, ab] = a;
    let [br, bg, bb] = b;
    let [cr, cg, cb] = c;
    let [dr, dg, db] = d;
    [
        (ar + br * (TAU * (cr * t + dr)).cos()).clamp(0.0, 1.0),
        (ag + bg * (TAU * (cg * t + dg)).cos()).clamp(0.0, 1.0),
        (ab + bb * (TAU * (cb * t + db)).cos()).clamp(0.0, 1.0),
    ]
}

/// Sample a sorted `(at, color)` stop list at `t`, clamping below the first / above
/// the last stop and linearly interpolating between the bracketing pair.
fn stops_at(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    let mut lo: Option<(f32, Rgb)> = None;
    for &(at, color) in stops {
        if at <= t {
            lo = Some((at, color));
        } else {
            // First stop past `t`: interpolate from `lo` (or clamp if none).
            let Some((lat, lcol)) = lo else {
                return color;
            };
            let span = (at - lat).max(1e-6);
            let f = ((t - lat) / span).clamp(0.0, 1.0);
            let [lr, lg, lb] = lcol;
            let [hr, hg, hb] = color;
            return [lr + (hr - lr) * f, lg + (hg - lg) * f, lb + (hb - lb) * f];
        }
    }
    // `t` is at or past the last stop: clamp to it (or black if the list is empty,
    // which the load boundary rejects — ≥2 stops).
    lo.map(|(_, color)| color).unwrap_or([0.0; 3])
}

/// Round a `[0, 1]` channel to an 8-bit value.
fn to_u8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::render::RenderError;
    use crate::render::context::RenderContext;

    /// **The deferred upload costs one `write_texture` pair per change and none
    /// otherwise** — the contract every scene's `render` leans on when it calls
    /// `flush` unconditionally on the hot path.
    ///
    /// A fresh pair is dirty because its textures hold no bytes yet, so the first
    /// flush after construction uploads. After that only a `set` can make one
    /// upload again, and two `set`s between frames still cost one — which is what
    /// makes a preset dissolve, which re-`set`s both sides every frame it runs,
    /// bounded rather than proportional to how often the palette is touched.
    ///
    /// Needs a GPU adapter to create the textures, so it skips on runners without
    /// one (ADR-0016).
    #[test]
    fn the_lut_pair_uploads_once_per_set_and_never_otherwise() {
        let ctx = match RenderContext::new_headless(16, 16, true) {
            Ok(ctx) => ctx,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
            Err(e) => panic!("headless context build failed: {e}"),
        };

        let mut luts = LutPair::new(&ctx.device, "lut-pair-test");
        assert!(
            luts.flush(&ctx.queue),
            "a fresh pair's textures are empty, so its first flush uploads"
        );
        assert!(
            !luts.flush(&ctx.queue),
            "nothing changed since, so the second flush uploads nothing"
        );

        luts.set(&Palette::bake(&PaletteConfig::Named(NamedPalette::Ember)));
        assert!(luts.flush(&ctx.queue), "one set, one upload");
        assert!(!luts.flush(&ctx.queue), "and only one");

        // Two sets between frames: still one upload, of the LAST palette set.
        luts.set(&Palette::bake(&PaletteConfig::Named(NamedPalette::Ice)));
        luts.set(&Palette::bake(&PaletteConfig::Named(NamedPalette::Mono)));
        assert!(luts.flush(&ctx.queue), "two sets still cost one upload");
        assert!(!luts.flush(&ctx.queue));

        // Setting the same palette again is still a set: the pair compares
        // nothing, deliberately — a 6 KB array comparison per switch would buy
        // an upload nobody measured as costly.
        let same = Palette::bake(&PaletteConfig::Named(NamedPalette::Mono));
        luts.set(&same);
        assert!(luts.flush(&ctx.queue));
    }

    /// The exact analytic cosine the fragment field / swarm used before this
    /// module — the no-regression reference.
    fn cosine_reference(t: f32) -> Rgb {
        cosine_at(
            [0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
            [0.10, 0.42, 0.62],
            t,
        )
    }

    /// The load-bearing no-regression guarantee (Plan 0020 Phase 1): the default
    /// `spectrum` palette baked into the LUT reproduces the prior analytic cosine
    /// (`d = 0.10, 0.42, 0.62`) within a small tolerance at several sampled `t`.
    /// That cosine is the one the **fragment field, swarm, and attractor** all
    /// used before this module, so this single assertion is their shared default-
    /// path no-regression proof (each also has a golden fixture within tolerance).
    /// Reaction-diffusion used a *different* cosine and was deliberately unified
    /// onto `spectrum` in Phase 5 (its golden baseline re-blessed), so it is the
    /// one scene whose default look intentionally changed. If this drifts, every
    /// shipped preset on those three scenes shifts color.
    #[test]
    fn spectrum_reproduces_the_prior_cosine() {
        let pal = Palette::default_spectrum();
        // Eight `t` across the range, including the fragment field's actual
        // operating band (field*0.6 -> [0, 0.6]) and the wrap edges.
        let samples = [0.0, 0.1, 0.2, 0.3, 0.45, 0.6, 0.75, 0.95];
        for &t in &samples {
            let got = pal.sample(t, 0.0);
            let want = cosine_reference(t);
            for k in 0..3 {
                assert!(
                    (got[k] - want[k]).abs() < 0.01,
                    "spectrum LUT drifts from the cosine at t={t} channel {k}: \
                     got {} want {}",
                    got[k],
                    want[k]
                );
            }
        }
    }

    /// The stop-list bake path (used by every named palette except `spectrum`):
    /// `mono` (black → white) is exact at the ends and linear between, so the
    /// midpoint is mid-gray. This exercises the same `bake_gradient` path the
    /// Phase 2 custom stops reuse.
    #[test]
    fn stops_interpolate_between_control_points() {
        let pal = Palette::bake(&PaletteConfig::Named(NamedPalette::Mono));
        let lo = pal.sample(0.002, 0.0);
        assert!(
            lo[0] < 0.05 && lo[1] < 0.05 && lo[2] < 0.05,
            "start ~black: {lo:?}"
        );
        let hi = pal.sample(0.998, 0.0);
        assert!(
            hi[0] > 0.95 && hi[1] > 0.95 && hi[2] > 0.95,
            "end ~white: {hi:?}"
        );
        let mid = pal.sample(0.5, 0.0);
        assert!(
            (mid[0] - 0.5).abs() < 0.05
                && (mid[1] - 0.5).abs() < 0.05
                && (mid[2] - 0.5).abs() < 0.05,
            "midpoint is mid-gray: {mid:?}"
        );
    }

    /// `saturation = 1` is identity; `saturation = 0` collapses to luma (gray);
    /// the shared definition both CPU and GPU use.
    #[test]
    fn saturation_endpoints() {
        let c = [0.8, 0.2, 0.1];
        let same = desaturate(c, 1.0);
        for k in 0..3 {
            assert!(
                (same[k] - c[k]).abs() < 1e-5,
                "saturation 1 is unchanged: {same:?} vs {c:?}"
            );
        }
        let gray = desaturate(c, 0.0);
        assert!(
            (gray[0] - gray[1]).abs() < 1e-6 && (gray[1] - gray[2]).abs() < 1e-6,
            "saturation 0 is gray: {gray:?}"
        );
    }

    /// The A/B crossfade (Plan 0020 Phase 4): `mix = 0` is exactly palette A,
    /// `mix = 1` is palette B, and `mix = 0.5` lands between — the bindable
    /// `palette_mix` behaviour, with the `mix = 0` = A-alone guarantee.
    #[test]
    fn palette_mix_crossfades_a_to_b() {
        // A = mono (black->white), B = a solid mid-gray via two equal stops, so at
        // a fixed `t` the two sides differ and the mix is easy to reason about.
        let a = PaletteConfig::Named(NamedPalette::Mono);
        let b = PaletteConfig::Named(NamedPalette::Ember);
        let pair = Palette::bake_pair(&a, &b);
        let a_only = Palette::bake(&a);

        let t = 0.85;
        // mix = 0 is exactly palette A alone (byte-for-byte with the single bake).
        assert_eq!(
            pair.sample(t, 0.0),
            a_only.sample(t, 0.0),
            "mix=0 is palette A alone"
        );
        // mix = 1 is palette B.
        let b_only = Palette::bake(&b);
        let at_one = pair.sample(t, 1.0);
        let want_b = b_only.sample(t, 0.0);
        for k in 0..3 {
            assert!((at_one[k] - want_b[k]).abs() < 1e-6, "mix=1 is palette B");
        }
        // mix = 0.5 is the midpoint of A and B per channel.
        let a_col = a_only.sample(t, 0.0);
        let mid = pair.sample(t, 0.5);
        for k in 0..3 {
            let expected = a_col[k] + (want_b[k] - a_col[k]) * 0.5;
            assert!(
                (mid[k] - expected).abs() < 1e-6,
                "mix=0.5 is the A/B midpoint"
            );
        }
    }

    // --- Banding (ADR-0078) ---------------------------------------------

    /// `palette_steps = N` leaves exactly `N` distinct palette coordinates over
    /// the gradient's range. Asserted on the CPU-side expression rather than on
    /// a capture, because a pixel count would also see the bloom, the backdrop
    /// and the 8-bit round-trip.
    #[test]
    fn six_steps_leave_exactly_six_palette_coordinates() {
        for n in [2.0f32, 4.0, 6.0, 8.0, 16.0] {
            let mut seen: Vec<f32> = Vec::new();
            // A dense sweep of the unit range, which is what a field level
            // multiplied by a `color_span` of 1 delivers.
            for i in 0..10_000 {
                let t = i as f32 / 10_000.0;
                let q = band_coord(t, n);
                if !seen.iter().any(|v| (v - q).abs() < 1e-6) {
                    seen.push(q);
                }
            }
            assert_eq!(
                seen.len(),
                n as usize,
                "palette_steps = {n} produced {} distinct coordinates, not {n}",
                seen.len()
            );
            // ...and each one is a band CENTRE, not an edge.
            for (k, q) in seen.iter().enumerate() {
                let _ = k;
                let centre = ((q * n).floor() + 0.5) / n;
                assert!(
                    (q - centre).abs() < 1e-6,
                    "quantized coordinate {q} is not a band centre at N = {n}"
                );
            }
        }
    }

    /// Off is the **exact** identity, which is what keeps every shipped preset
    /// and every golden baseline byte-identical. Not approximately: the
    /// coordinate is returned untouched rather than run through a one-band
    /// quantization, which would snap the whole palette to a single colour.
    #[test]
    fn banding_below_two_steps_is_the_exact_identity() {
        for steps in [0.0f32, 1.0] {
            for i in -50..150 {
                let t = i as f32 / 100.0;
                assert_eq!(
                    band_coord(t, steps),
                    t,
                    "palette_steps = {steps} is not the identity at t = {t}"
                );
            }
        }
        // The quantization does reach a coordinate at 2, or the above is a
        // statement about a function that never does anything.
        assert_ne!(band_coord(0.1, 2.0), 0.1);
    }

    /// The band count never reaches a sample site fractional, for
    /// `kaleidoscope.rs`'s `fold_order` reason: an eased binding sweeps
    /// continuously, and a fractional band count leaves every boundary crawling
    /// rather than stepping.
    #[test]
    fn band_steps_is_always_integral_and_in_range() {
        for &raw in &[-9.0f32, 0.0, 0.4, 3.5, 6.0, 6.4, 6.6, 1e9] {
            let n = band_steps(raw);
            assert_eq!(n, n.round(), "band_steps({raw}) = {n} is not an integer");
            assert!((0.0..=MAX_PALETTE_STEPS).contains(&n));
        }
        assert_eq!(band_steps(6.4), 6.0);
        assert_eq!(band_steps(6.6), 7.0);
        assert_eq!(band_steps(f32::NAN), DEFAULT_PALETTE_STEPS);
        assert_eq!(band_steps(f32::INFINITY), DEFAULT_PALETTE_STEPS);
        assert_eq!(band_contour(2.0), 1.0);
        assert_eq!(band_contour(-1.0), 0.0);
        assert_eq!(band_contour(f32::NAN), DEFAULT_PALETTE_CONTOUR);
    }

    // --- The WGSL copies have not drifted --------------------------------
    //
    // ADR-0078's accepted cost: this project has no shader include mechanism, so
    // the banding expression is a commented verbatim copy at every WGSL sample
    // site, exactly as `apply_saturation` mirrors `desaturate`. This is the
    // mitigation, and it is weaker than not having copies — it can only see that
    // the copies agree with the text below, not that the text is right.
    //
    // The scene sources are pulled in with `include_str!` rather than read from
    // disk, so a moved or renamed file fails to COMPILE here instead of silently
    // checking nothing.

    /// The canonical WGSL banding function. Must appear byte-for-byte in every
    /// shader that samples the LUT.
    const BAND_COORD_WGSL: &str = "\
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}";

    /// The canonical WGSL contour function. **Fragment stage only** — it calls
    /// `fwidth`.
    const BAND_CONTOUR_WGSL: &str = "\
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
}";

    const FRAGMENT_FIELD_SRC: &str = include_str!("scenes/fragment_field.rs");
    const REACTION_DIFFUSION_SRC: &str = include_str!("scenes/reaction_diffusion.rs");
    const PARTICLE_SHADERS_SRC: &str = include_str!("scenes/particles/shaders.rs");
    const SHAPE_FIELD_SRC: &str = include_str!("scenes/shape_field.rs");
    /// The **fourth** contour site. It was missing from the list below until Plan
    /// 0121 Phase 5, and its copy had drifted (`dd` for `d`) — so the test that
    /// exists to catch drift could not have caught this one, because the site it
    /// lived at was never iterated.
    const WARP_MESH_SRC: &str = include_str!("scenes/warp_mesh/shaders.rs");

    #[test]
    fn every_wgsl_sample_site_carries_the_same_banding_expression() {
        for (name, src) in [
            ("fragment_field.rs", FRAGMENT_FIELD_SRC),
            ("reaction_diffusion.rs", REACTION_DIFFUSION_SRC),
            ("particles/shaders.rs", PARTICLE_SHADERS_SRC),
            ("shape_field.rs", SHAPE_FIELD_SRC),
        ] {
            assert!(
                src.contains(BAND_COORD_WGSL),
                "{name}'s copy of the WGSL `band_coord` has drifted from \
                 palette.rs::band_coord — the two must stay one function written \
                 twice, not two functions that agree on some inputs"
            );
        }
    }

    /// ...and the contour reaches the **fragment-stage** scenes only. Asserted,
    /// not merely documented: the attractor's LUT read is in the vertex stage,
    /// where `fwidth` does not exist, so a copy landing there is a compile error
    /// at best and a silent nothing at worst.
    #[test]
    fn the_contour_reaches_the_fragment_sites_and_not_the_vertex_one() {
        for (name, src) in [
            ("fragment_field.rs", FRAGMENT_FIELD_SRC),
            ("reaction_diffusion.rs", REACTION_DIFFUSION_SRC),
            ("shape_field.rs", SHAPE_FIELD_SRC),
            ("warp_mesh/shaders.rs", WARP_MESH_SRC),
        ] {
            assert!(
                src.contains(BAND_CONTOUR_WGSL),
                "{name}'s copy of the WGSL `band_contour` has drifted"
            );
        }
        assert!(
            !PARTICLE_SHADERS_SRC.contains("fn band_contour"),
            "particles/shaders.rs grew a `band_contour` — its LUT read is in the \
             VERTEX stage, which has no derivatives and no gradient across a point \
             sprite to contour (ADR-0078)"
        );
    }
}
