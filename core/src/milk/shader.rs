//! The converted-shader interface (Plan 0100 Phase 6): what a translated
//! MilkDrop 2 `warp`/`comp` fragment shader may bind, the uniform block behind
//! its ~40-name input surface, and the naga gate every bundle shader passes at
//! load.
//!
//! # This is the contract, in one place
//!
//! `milkconv` emits a complete WGSL fragment module: [`fragment_prelude`] (the
//! bindings and helper functions below) followed by the preset's translated
//! code. The engine builds the matching pipeline from the same constants —
//! [`BINDINGS`], the group indices, the varying locations — in
//! `render/scenes/warp_mesh/shader.rs`. Keeping both halves keyed off this one
//! module is what stops the converter and the engine drifting into two
//! incompatible interfaces that fail only at pipeline creation.
//!
//! **No HLSL and no translator is here** (ADR-0113): what this module knows is
//! the *WGSL* surface, which is as much a runtime interface as the C ABI is.
//!
//! # Why validation happens twice
//!
//! [`validate_wgsl`] runs in `milkconv` the moment a shader is translated — so
//! an emitter bug is a named conversion failure in Phase 5's ranking — and again
//! in the preset loader, because a bundle on disk is untrusted text and the
//! boundary rule (validate at the boundary, trust inside) applies to it exactly
//! as it applies to sample rates. **A failed compile rejects that preset by
//! name and loads the rest** — the directory loader already skips a bad preset
//! per file, so the second check needs no new machinery.
//!
//! naga itself is not a new dependency: wgpu compiles every shader in this
//! engine through it already, and `wgpu::naga` re-exports the same version.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to core/src/milk by
// Plan 0100 Phase 2). Validation runs at load, but the pragma is the module
// convention.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The bind-group slot a converted **warp** fragment shader uses. Group 0 is the
/// warp pass's own vertex-stage uniform (the mesh transform), so the shader
/// surface sits beside it.
pub const WARP_GROUP: u32 = 1;
/// The bind-group slot a converted **comp** fragment shader uses. The fullscreen
/// vertex prelude binds nothing, so the shader surface is the only group.
pub const COMP_GROUP: u32 = 0;

/// The binding roster of the shader surface, in binding order. The names are the
/// WGSL identifiers [`fragment_prelude`] declares; the engine's bind-group
/// layout mirrors this list positionally.
pub const BINDINGS: &[&str] = &[
    "U",               // 0: the uniform block below
    "t_main",          // 1: the field — the past for warp, this frame for comp
    "s_fw",            // 2: filtering + wrap
    "s_fc",            // 3: filtering + clamp
    "s_pw",            // 4: point + wrap
    "s_pc",            // 5: point + clamp
    "t_noise_lq",      // 6
    "t_noise_lq_lite", // 7
    "t_noise_mq",      // 8
    "t_noise_hq",      // 9
    "t_noisevol_lq",   // 10 (3D)
    "t_noisevol_hq",   // 11 (3D)
    "t_blur1",         // 12
    "t_blur2",         // 13
    "t_blur3",         // 14
];

/// How many `rot_*` matrices the surface carries: the six families MilkDrop
/// declares (`s`, `d`, `f`, `vf`, `uf`, `rand`), four of each. Stored as four
/// `vec4` rows per matrix — the shape `float4x3` indexes as.
pub const ROT_MATRICES: usize = 24;

/// **The feedback-field quantizer** ([ADR-0118](../../../docs/adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md)),
/// as WGSL — the *one* text, emitted into every converted module by
/// [`fragment_prelude`] and concatenated onto the engine's own built-in warp
/// fragment by `render/scenes/warp_mesh`. A transfer function written out twice
/// is a transfer function that drifts.
///
/// # Why the round trip
///
/// The reference's feedback target is 8-bit, so `decay` times a dim pixel
/// **truncates to zero** and a classic preset's background stays black. This
/// engine's field is `Rgba16Float`, nothing truncates, and every dim residual
/// integrates — the wash, glow, runaway and inversion Plan 0100 Phase 7 judged.
///
/// The field is **linear light** (ADR-0046) and the reference quantizes in its
/// **gamma-encoded** target, so the step has to be taken in the encoded domain.
/// One number carries the whole decision: one 8-bit sRGB step is `1/255 =
/// 0.00392` encoded, which is `3.03e-4` in linear light. A literal `1/255` floor
/// applied *in linear* would truncate everything below encoded `0.0498` — sRGB
/// level ~13, **thirteen times too aggressive** — and would crush the dim trails
/// the reference keeps rather than the dimmer ones it discards.
///
/// # Why sRGB and not the reference's own ~2.2 gamma
///
/// DX9-era MilkDrop wrote to an 8-bit target with no explicit encoding, which in
/// practice is a plain 2.2 power curve — and it differs from sRGB's piecewise
/// one *only* in the near-black region this whole decision is about, so the
/// choice was rendered rather than assumed (ADR-0118's `Outcome`, 2026-08-17).
/// The two agree to within one 8-bit level at every frame where the picture
/// reads. They part in the tail, and there sRGB's floor lands on linear
/// `3.03e-4` — **exactly 8-bit display level 1**, so what it discards is
/// precisely what a viewer could not have seen. The 2.2 curve floors at
/// `(1/255)^2.2 = 5.1e-6`, 59x lower, keeping six more e-foldings of invisible
/// light alive to accumulate. sRGB is also already here (ADR-0046).
///
/// # The lane
///
/// `steps` is a **count**, not a flag, so the look gate can A/B a tuning without
/// a rebuild:
///
/// - `|steps| < 1` — off, and off is an **exact identity**: the early return
///   hands back the argument, so a native `warp_mesh` preset renders the bytes it
///   rendered before this existed.
/// - `steps > 0` — the decision: encode, floor to `steps` levels, decode.
/// - `steps < 0` — ADR-0118's **Alternative D**, the named fallback: floor to
///   zero at one encoded step and leave the levels between alone. It is the half
///   of the mechanism that does the visible work (dim residuals die instead of
///   accumulating) without re-introducing the banding ADR-0096 dithers away.
///   Reachable from the same lane on purpose — the fallback is a parameter
///   change, not a rebuild.
pub const QUANTIZE_WGSL: &str = "\
fn lmv_srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let lo = x * 12.92;
    let hi = 1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, x <= vec3<f32>(0.0031308));
}
fn lmv_srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    let lo = x / 12.92;
    let hi = pow((x + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, x <= vec3<f32>(0.04045));
}
fn lmv_quantize(c: vec3<f32>, steps: f32) -> vec3<f32> {
    let n = abs(steps);
    if (n < 1.0) { return c; }
    let e = lmv_srgb_encode(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
    if (steps < 0.0) {
        return select(c, vec3<f32>(0.0), e < vec3<f32>(1.0 / n));
    }
    return lmv_srgb_decode(floor(e * n) / n);
}
";

/// The uniform block, as WGSL. **Field-for-field with `MilkUniform` in
/// `render/scenes/warp_mesh/shader.rs`** — every member is 16-byte data, so the
/// Rust `#[repr(C)]` layout and the WGSL std140-ish layout agree by
/// construction, and a test naga-parses this text so the two cannot drift
/// silently.
pub const UNIFORM_WGSL: &str = "\
struct MilkU {
    // x: time (s), y: fps (the nominal 30, as the EEL side reports), z: frame, w: progress
    clock: vec4<f32>,
    // bass, mid, treb, vol — MilkDrop-scaled, exactly what the EEL programs read
    bands: vec4<f32>,
    // the attenuated versions of the same four
    bands_att: vec4<f32>,
    // w, h, 1/w, 1/h of the field
    texsize: vec4<f32>,
    // aspectx, aspecty, 1/aspectx, 1/aspecty — the EEL convention (longer axis reads 1)
    aspect: vec4<f32>,
    // four uniform randoms, fresh each frame, deterministic from the preset salt
    rand_frame: vec4<f32>,
    // four uniform randoms, fixed for the preset's life
    rand_preset: vec4<f32>,
    // x: decay for this frame (already rate-converted), y: brightness, z: occlude,
    // w: feedback quantize steps (0 = off, negative = ADR-0118 Alternative D)
    misc: vec4<f32>,
    // the four corner colours hue_shader interpolates between
    hue: array<vec4<f32>, 4>,
    // q1..q32 as _qa.._qh
    q: array<vec4<f32>, 8>,
    // slow_roam_cos, roam_cos, slow_roam_sin, roam_sin
    roam: array<vec4<f32>, 4>,
    // the 24 rot_* matrices, 4 rows each; .xyz of a row is what float4x3 indexing reads
    rot: array<vec4<f32>, 96>,
}
";

/// The whole fixed half of a converted fragment module: the uniform block, the
/// binding declarations at `group`, and MilkDrop's own prelude helpers
/// (`lum`, `GetPixel`, `GetBlur1..3`) under collision-proof `lmv_` names.
///
/// Helpers are declared whether or not the preset calls them — an uncalled
/// function does not put its bindings in the entry point's resource set, so a
/// shader that never blurs needs no blur textures in its layout.
pub fn fragment_prelude(group: u32) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str(UNIFORM_WGSL);
    let g = group;
    let decls = [
        format!("@group({g}) @binding(0) var<uniform> U: MilkU;"),
        format!("@group({g}) @binding(1) var t_main: texture_2d<f32>;"),
        format!("@group({g}) @binding(2) var s_fw: sampler;"),
        format!("@group({g}) @binding(3) var s_fc: sampler;"),
        format!("@group({g}) @binding(4) var s_pw: sampler;"),
        format!("@group({g}) @binding(5) var s_pc: sampler;"),
        format!("@group({g}) @binding(6) var t_noise_lq: texture_2d<f32>;"),
        format!("@group({g}) @binding(7) var t_noise_lq_lite: texture_2d<f32>;"),
        format!("@group({g}) @binding(8) var t_noise_mq: texture_2d<f32>;"),
        format!("@group({g}) @binding(9) var t_noise_hq: texture_2d<f32>;"),
        format!("@group({g}) @binding(10) var t_noisevol_lq: texture_3d<f32>;"),
        format!("@group({g}) @binding(11) var t_noisevol_hq: texture_3d<f32>;"),
        format!("@group({g}) @binding(12) var t_blur1: texture_2d<f32>;"),
        format!("@group({g}) @binding(13) var t_blur2: texture_2d<f32>;"),
        format!("@group({g}) @binding(14) var t_blur3: texture_2d<f32>;"),
    ];
    for d in decls {
        out.push_str(&d);
        out.push('\n');
    }
    out.push_str(
        "\nfn lmv_lum(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.32, 0.49, 0.29)); }\n\
         fn lmv_GetPixel(uv: vec2<f32>) -> vec3<f32> {\n\
         \x20   return textureSampleLevel(t_main, s_fc, uv, 0.0).xyz;\n\
         }\n\
         fn lmv_GetBlur1(uv: vec2<f32>) -> vec3<f32> {\n\
         \x20   return textureSampleLevel(t_blur1, s_fc, uv, 0.0).xyz;\n\
         }\n\
         fn lmv_GetBlur2(uv: vec2<f32>) -> vec3<f32> {\n\
         \x20   return textureSampleLevel(t_blur2, s_fc, uv, 0.0).xyz;\n\
         }\n\
         fn lmv_GetBlur3(uv: vec2<f32>) -> vec3<f32> {\n\
         \x20   return textureSampleLevel(t_blur3, s_fc, uv, 0.0).xyz;\n\
         }\n\n",
    );
    // The feedback quantizer, declared for both stages though only the warp
    // epilogue calls it — the same "helpers are declared whether or not the
    // preset calls them" rule as above, and it keeps the two stages' preludes
    // one text.
    out.push_str(QUANTIZE_WGSL);
    out.push('\n');
    out
}

/// Parse and validate one WGSL module through naga — the same frontend wgpu
/// hands every shader in this engine to, so passing here is passing the real
/// gate rather than a lookalike.
///
/// The error is a `String` because both callers (the converter's per-preset
/// ranking and the loader's per-file skip) want text with the preset's name
/// wrapped around it, not a type to match on.
pub fn validate_wgsl(source: &str) -> Result<(), String> {
    use wgpu::naga;
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| e.emit_to_string_with_path(source, "shader"))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .map_err(|e| e.emit_to_string_with_path(source, "shader"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Test asserts panic on failure; allowed over the file pragma.
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    /// The uniform block and the prelude are valid WGSL on their own — the
    /// converter concatenates translated code after them, so a syntax error here
    /// would surface as "every preset fails", attributed to the wrong side.
    #[test]
    fn the_prelude_is_valid_wgsl_at_both_groups() {
        for group in [WARP_GROUP, COMP_GROUP] {
            let module = format!(
                "{}\n@fragment fn fs_main() -> @location(0) vec4<f32> {{\n\
                 \x20   return vec4<f32>(U.clock.x, lmv_lum(lmv_GetPixel(vec2<f32>(0.5))), 0.0, 1.0);\n\
                 }}\n",
                fragment_prelude(group)
            );
            validate_wgsl(&module).unwrap();
        }
    }

    /// The gate actually gates: junk and *valid-but-broken* WGSL are both
    /// refused with text a loader can print.
    #[test]
    fn validate_refuses_what_naga_refuses() {
        assert!(validate_wgsl("this is not wgsl").is_err());
        // Parses, but the entry point returns the wrong type — a validation
        // failure rather than a parse failure, so both halves of the gate run.
        assert!(
            validate_wgsl("@fragment fn fs_main() -> @location(0) vec4<f32> { return 1.0; }")
                .is_err()
        );
    }

    /// The binding roster and the prelude declare the same surface — the
    /// engine's layout is built positionally from [`BINDINGS`].
    #[test]
    fn the_roster_matches_the_prelude() {
        let prelude = fragment_prelude(WARP_GROUP);
        for (index, name) in BINDINGS.iter().enumerate() {
            assert!(
                prelude.contains(&format!("@binding({index}) var")),
                "binding {index} missing from the prelude"
            );
            assert!(
                prelude.contains(&format!(" {name}:")),
                "`{name}` missing from the prelude"
            );
        }
    }
}
