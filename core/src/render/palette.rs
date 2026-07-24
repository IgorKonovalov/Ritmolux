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
//! **The `spectrum` default is the exact current cosine**, so a preset that
//! declares no `[palette]` renders identically to before this module existed —
//! the shipped presets are unchanged until re-authored (the load-bearing
//! no-regression guarantee, gated by a unit test comparing sampled colors).
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

/// One RGB entry — display-space values in `[0, 1]`, used directly as color
/// (no perceptual/gamma management; that is deferred, ADR-0021 Alt E).
pub type Rgb = [f32; 3];

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
/// [`Palette::bake`] (validate-at-the-boundary). Phase 2 adds a custom-stops
/// variant here.
#[derive(Debug, Clone)]
pub enum PaletteConfig {
    /// A built-in named palette.
    Named(NamedPalette),
}

impl PaletteConfig {
    /// The default when a preset declares no `[palette]` — the exact current
    /// cosine, so shipped presets are unchanged.
    pub fn default_spectrum() -> Self {
        PaletteConfig::Named(NamedPalette::Spectrum)
    }
}

/// One gradient baked into a 256-entry RGB LUT. Sampled on the GPU (as a 256×1
/// texture) and on the CPU (via [`sample`](Palette::sample)) from the same table.
/// `Copy` (a 3 KB array) so a scene can hold its own copy for deferred upload.
#[derive(Clone, Copy)]
pub struct Palette {
    lut: [Rgb; LUT_SIZE],
}

impl Palette {
    /// Bake `cfg` into the LUT. Pure; off the hot path (preset load only).
    pub fn bake(cfg: &PaletteConfig) -> Palette {
        let lut = match cfg {
            PaletteConfig::Named(named) => bake_gradient(&named.gradient()),
        };
        Palette { lut }
    }

    /// The default palette (`spectrum`), used when a preset declares no
    /// `[palette]` table.
    pub fn default_spectrum() -> Palette {
        Palette::bake(&PaletteConfig::default_spectrum())
    }

    /// Sample the LUT at `t`, linearly interpolated with the same texel-center
    /// convention (and wrap) the GPU texture sampler uses, so the CPU (swarm) and
    /// GPU scenes color consistently. Allocation-free — the swarm calls this per
    /// particle per frame. `t` outside `[0, 1)` wraps (the gradient repeats, like
    /// the cosine's periodic hue wheel).
    pub fn sample(&self, t: f32) -> Rgb {
        // Texel centers sit at (i + 0.5)/N (matching `bake_gradient` and hardware
        // filtering), so map `t` to x = t*N - 0.5 and lerp the bracketing texels.
        let tw = t - t.floor(); // wrap to [0, 1)
        let x = tw * LUT_SIZE as f32 - 0.5;
        let i0 = x.floor().rem_euclid(LUT_SIZE as f32) as usize;
        let i1 = (i0 + 1) % LUT_SIZE;
        let frac = x - x.floor();
        let a = self.lut.get(i0).copied().unwrap_or([0.0; 3]);
        let b = self.lut.get(i1).copied().unwrap_or([0.0; 3]);
        let [ar, ag, ab] = a;
        let [br, bg, bb] = b;
        [
            ar + (br - ar) * frac,
            ag + (bg - ag) * frac,
            ab + (bb - ab) * frac,
        ]
    }

    /// The baked LUT as tight RGBA8 bytes for a 256×1 `Rgba8Unorm` texture upload
    /// (the GPU scenes). Alpha is opaque; the display surface is 8-bit, so 8-bit
    /// LUT storage adds no visible banding over the analytic cosine.
    pub fn rgba8_bytes(&self) -> [u8; LUT_SIZE * 4] {
        let mut out = [0u8; LUT_SIZE * 4];
        for (px, rgb) in out.chunks_exact_mut(4).zip(self.lut.iter()) {
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
        label: Some("lmv-lut-sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// Upload a baked palette's LUT into its 256×1 texture. Off the hot path — called
/// from a scene's deferred `set_palette` upload (first frame after a preset
/// switch).
pub fn write_lut(queue: &wgpu::Queue, texture: &wgpu::Texture, palette: &Palette) {
    let bytes = palette.rgba8_bytes();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
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
    /// within a small tolerance at several sampled `t`. If this drifts, every
    /// shipped preset's color shifts.
    #[test]
    fn spectrum_reproduces_the_prior_cosine() {
        let pal = Palette::default_spectrum();
        // Eight `t` across the range, including the fragment field's actual
        // operating band (field*0.6 -> [0, 0.6]) and the wrap edges.
        let samples = [0.0, 0.1, 0.2, 0.3, 0.45, 0.6, 0.75, 0.95];
        for &t in &samples {
            let got = pal.sample(t);
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
        let lo = pal.sample(0.002);
        assert!(
            lo[0] < 0.05 && lo[1] < 0.05 && lo[2] < 0.05,
            "start ~black: {lo:?}"
        );
        let hi = pal.sample(0.998);
        assert!(
            hi[0] > 0.95 && hi[1] > 0.95 && hi[2] > 0.95,
            "end ~white: {hi:?}"
        );
        let mid = pal.sample(0.5);
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
}
