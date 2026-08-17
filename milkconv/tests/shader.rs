//! The HLSL→WGSL conformance suite (Plan 0100 Phase 6).
//!
//! Same placement logic as `conformance.rs` and `draw_layer.rs`: only this
//! crate can translate HLSL, so the tests that ask "does the translation mean
//! what the source meant" live here — and every emitted module is pushed
//! through `lmv_core::milk::shader::validate_wgsl`, which is the *same* naga
//! frontend the engine loads bundles through, so "validates" here is the real
//! gate rather than a lookalike.

use milkconv::shader::{Stage, translate};

/// Translate a warp-stage snippet and insist naga accepts the module.
fn warp(src: &str) -> milkconv::shader::Translated {
    let translated = translate(Stage::Warp, src, true)
        .unwrap_or_else(|e| panic!("must translate: {e}\n---\n{src}"));
    lmv_core::milk::shader::validate_wgsl(&translated.wgsl)
        .unwrap_or_else(|e| panic!("must validate: {e}\n---\n{}", translated.wgsl));
    translated
}

/// **The corpus's idioms translate and validate.** Each entry is a shape that
/// appears verbatim (modulo names) in the 10 347-file corpus; together they
/// cover the intrinsics, the swizzle forms, the implicit conversions, the
/// control flow and the input surface the census priced Phase 6 by.
#[test]
fn the_corpus_idioms_translate_and_validate() {
    let fixtures: &[&str] = &[
        // The default warp shader, near enough.
        "shader_body { ret = tex2D(sampler_main, uv).xyz * decay; }",
        // Swizzles, including rgba names and scalar broadcast.
        "shader_body { float3 c = tex2D(sampler_main, uv).rgb; float k = c.g; ret = k.xxx; }",
        // The rotation-matrix idiom, with mul in both orders.
        "shader_body {\n\
         float a = time*0.1;\n\
         float2x2 rot = float2x2(cos(a), sin(a), -sin(a), cos(a));\n\
         float2 uv2 = mul(rot, uv - 0.5) + 0.5;\n\
         float2 uv3 = mul(uv - 0.5, rot) + 0.5;\n\
         ret = tex2D(sampler_main, uv2).xyz + tex2D(sampler_main, uv3).xyz;\n\
         }",
        // Ternaries, float conditions, boolean arithmetic.
        "shader_body {\n\
         float k = (bass > 1.2) ? 1 : 0.2;\n\
         if (treb) { k += 0.1; }\n\
         ret = GetPixel(uv) * k * (1 + (mid > 1)*0.5);\n\
         }",
        // A bounded for loop with an int counter, compound assignment, q vars.
        "shader_body {\n\
         float3 acc = 0;\n\
         for (int i = 0; i < 8; i++) acc += GetPixel(uv + float2(i*0.001, q1*0.01));\n\
         ret = acc / 8.0;\n\
         }",
        // A while loop with break and continue.
        "shader_body {\n\
         float r = rad;\n\
         while (r > 0.1) { r *= 0.5; if (r > 0.4) continue; if (r < 0.2) break; }\n\
         ret = float3(r, ang, 0);\n\
         }",
        // A parameterless #define and a user helper function.
        "#define HALF 0.5\n\
         float3 tint(float3 c, float k) { return c * k; }\n\
         shader_body { ret = tint(GetPixel(uv), HALF); }",
        // The noise samplers, 2D and 3D, and the texsize constants.
        "shader_body {\n\
         float3 n = tex2D(sampler_noise_lq, uv*texsize.xy*texsize_noise_lq.zw).xyz;\n\
         float3 v = tex3D(sampler_noisevol_lq, float3(uv, frac(time*0.1))).xyz;\n\
         ret = n * 0.5 + v * 0.5;\n\
         }",
        // The blur helpers and lum.
        "shader_body { ret = GetBlur2(uv) * lum(GetPixel(uv)); }",
        // The roam vectors, rand vectors, hue_shader, aspect.
        "shader_body {\n\
         float2 p = uv * aspect.xy + slow_roam_cos.xy * 0.01 + rand_preset.xy * 0.001;\n\
         ret = GetPixel(p) * hue_shader * (0.9 + 0.1*roam_sin.z);\n\
         }",
        // rot_* row indexing — the shape the 28 files that touch them use.
        "shader_body { ret = GetPixel(uv + 0.01*rot_d2[1].xy); }",
        // Write-masked swizzle assignment, both plain and compound.
        "shader_body {\n\
         float4 c = tex2D(sampler_main, uv);\n\
         c.xy = c.yx * 0.5;\n\
         c.rgb += 0.01;\n\
         ret = c.xyz;\n\
         }",
        // Vector truncation and scalar-to-vector init, fxc style.
        "shader_body {\n\
         float2 a = tex2D(sampler_main, uv);\n\
         float3 b = 0;\n\
         b = float3(a, lum(GetPixel(uv)));\n\
         ret = b;\n\
         }",
        // The two-argument atan overload and the point samplers.
        "shader_body {\n\
         float t = atan(uv.y - 0.5, uv.x - 0.5);\n\
         ret = tex2D(sampler_pc_main, uv + 0.001*sin(t)).xyz;\n\
         }",
        // The comparison intrinsics zoo.
        "shader_body {\n\
         float3 c = GetPixel(uv);\n\
         c = lerp(c, saturate(pow(c, 1.8)), 0.5);\n\
         c = clamp(c, 0.0, 1.0) + fmod(time, 2.0)*0.001;\n\
         c = max(c, min(c*2, sqrt(abs(c))));\n\
         ret = normalize(float3(c.x, c.y, 1)) * length(c) + frac(c) * 0.001;\n\
         }",
        // A global (mutable static, fxc-style) and multi-declarators.
        "float speed = 2;\n\
         shader_body { float a = 1, b = 2; ret = GetPixel(uv) * (a + b) * speed * 0.1; }",
        // The four constructs the original pack's failure ranking surfaced,
        // verbatim shapes from the files that failed (2026-08-16):
        // `//*` starting a line comment must not open a block comment.
        "shader_body { ret = GetPixel(uv); ret -= 0.02;//*= 0.95; //or try\n ret /= 1.1; }",
        // A C brace initializer, the corpus's favourite rotation spelling.
        "shader_body {\n\
         float2x2 rot = { cos(q9), sin(q9),\n\
                          -sin(q9), cos(q9) };\n\
         ret = tex2D(sampler_main, mul(uv - 0.5, rot) + 0.5).xyz;\n\
         }",
        // The comma operator, lowercase tex2d, a prefixed noise sampler, and a
        // mixed scalar/vector matrix constructor.
        "shader_body {\n\
         float2 uv2 = uv + texsize.zx*(q3,q3);\n\
         float3 c = tex2d(sampler_pw_noise_lq, uv2) - 0.5;\n\
         float2x2 m = float2x2(c.xy, -c.y, c.x);\n\
         ret = float3(mul(m, uv2), lum(c) * M_INV_PI_2 * M_PI_2);\n\
         }",
    ];
    for src in fixtures {
        warp(src);
    }
}

/// A comp-stage module validates too — its group index, varyings and epilogue
/// differ from the warp stage's, so both shapes have to pass.
#[test]
fn a_comp_stage_module_validates() {
    let translated = translate(
        Stage::Comp,
        "shader_body { ret = GetPixel(uv) * hue_shader + GetBlur3(uv)*0.3; }",
        false,
    )
    .expect("translates");
    lmv_core::milk::shader::validate_wgsl(&translated.wgsl).expect("validates");
    assert_eq!(translated.blur_level, 3);
}

/// **The matrix constructor keeps HLSL's mathematical matrix.** HLSL fills
/// rows, WGSL fills columns; the emitted `transpose` is what lets `mul`
/// translate positionally without silently transposing every rotation in the
/// corpus.
#[test]
fn the_matrix_constructor_goes_through_transpose() {
    let translated = warp(
        "shader_body {\n\
         float2x2 m = float2x2(1, 2, 3, 4);\n\
         ret = float3(mul(m, uv), 0);\n\
         }",
    );
    assert!(
        translated.wgsl.contains("transpose(mat2x2<f32>("),
        "rows must be wrapped in a transpose:\n{}",
        translated.wgsl
    );
}

/// **Every loop is bounded** — the one lever against a converted shader
/// tripping a driver watchdog. The guard is in the emitted text, and the count
/// is recorded for the bundle header.
#[test]
fn every_loop_carries_its_guard() {
    let translated = warp(
        "shader_body {\n\
         float k = 0;\n\
         while (k < bass) { k += 0.1; }\n\
         for (int i = 0; i < 4; i++) k *= 1.1;\n\
         ret = k.xxx;\n\
         }",
    );
    assert_eq!(translated.loops, 2);
    assert_eq!(
        translated.wgsl.matches("< 1024").count(),
        2,
        "each loop gets its own iteration guard:\n{}",
        translated.wgsl
    );
}

/// Nesting past the cap is a named rejection, not an emitted hazard.
#[test]
fn loops_nested_past_the_cap_reject() {
    let err = translate(
        Stage::Warp,
        "shader_body {\n\
         for (int i = 0; i < 4; i++)\n\
         \x20 for (int j = 0; j < 4; j++)\n\
         \x20   for (int k = 0; k < 4; k++)\n\
         \x20     ret += 0.01;\n\
         }",
        true,
    )
    .expect_err("three nested loops exceed the cap");
    assert_eq!(err.class, "too-big");
}

/// **A disk texture rejects with the class the plan prices.** This is the
/// deliberate 19 %-of-corpus exclusion; the sampler's own name is in the
/// message so Phase 5's ranking and a human both see what was asked for.
#[test]
fn a_disk_texture_rejects_by_name() {
    for src in [
        "sampler sampler_flexi;\nshader_body { ret = 0; }",
        "shader_body { ret = tex2D(sampler_myimage, uv).xyz; }",
        "shader_body { ret = texsize_headlight.xyz; }",
    ] {
        let err = translate(Stage::Warp, src, true).expect_err(src);
        assert_eq!(err.class, "disk-texture", "{src}: {}", err.message);
    }
}

/// An unknown name is its own class — the no-silent-zero rule, shader flavour.
#[test]
fn an_unknown_name_rejects_with_its_class() {
    let err = translate(
        Stage::Warp,
        "shader_body { ret = mystery_input.xxx; }",
        true,
    )
    .expect_err("rejects");
    assert_eq!(err.class, "unknown-name");
    assert!(err.message.contains("mystery_input"));
}

/// **Writing a shader input is legal, because fxc made it legal**: to the
/// reference's compiler `rand_preset` is an ordinary mutable global, and
/// shipped presets assign to it. The translation shadows the input into a
/// `var<private>` filled from the uniform, so reads before the write still see
/// the real value.
#[test]
fn an_assigned_input_gets_a_writable_shadow() {
    let translated = warp(
        "shader_body {\n\
         float3 a = rand_preset.xyz;\n\
         rand_preset = float4(0.5, 0.5, 0.5, 0.5);\n\
         ret = a + rand_preset.xyz;\n\
         }",
    );
    assert!(
        translated
            .wgsl
            .contains("var<private> m_rand_preset: vec4<f32>;")
    );
    assert!(translated.wgsl.contains("m_rand_preset = U.rand_preset;"));
}

/// `sampler_main` follows the preset's `bTexWrap`, exactly as the reference
/// binds it: wrap on means the wrap sampler.
#[test]
fn sampler_main_honours_tex_wrap() {
    let src = "shader_body { ret = tex2D(sampler_main, uv).xyz; }";
    let wrapped = translate(Stage::Warp, src, true).expect("translates");
    let clamped = translate(Stage::Warp, src, false).expect("translates");
    assert!(wrapped.wgsl.contains("textureSampleLevel(t_main, s_fw"));
    assert!(clamped.wgsl.contains("textureSampleLevel(t_main, s_fc"));
}

/// `tex2D` never becomes `textureSample`: implicit derivatives inside the
/// conditionals presets sample from would fail naga's uniformity analysis, and
/// no MilkDrop texture has mips for level 0 to miss.
#[test]
fn sampling_is_always_explicit_level_zero() {
    let translated = warp(
        "shader_body {\n\
         if (bass > 1) { ret = tex2D(sampler_main, uv).xyz; }\n\
         else { ret = GetBlur1(uv); }\n\
         }",
    );
    assert!(!translated.wgsl.contains("textureSample("));
}

// ---------------------------------------------------------------------------
// End to end: a MilkDrop 2 preset with both shaders
// ---------------------------------------------------------------------------

/// A minimal MilkDrop 2 preset whose warp and comp shaders are both real work:
/// the warp displaces and decays, the comp tints through `hue_shader` and blurs.
const MD2_PRESET: &str = "\
[preset00]
MILKDROP_PRESET_VERSION=201
fRating=3.000
fDecay=0.96
bTexWrap=1
nWaveMode=0
fWaveAlpha=1.0
per_frame_1=zoom = 1.002;
warp_1=`shader_body
warp_2=`{
warp_3=`    float2 uv2 = uv + float2(0.003*sin(time*2 + uv.y*12), 0);
warp_4=`    ret = tex2D(sampler_main, uv2).xyz * decay;
warp_5=`}
comp_1=`shader_body
comp_2=`{
comp_3=`    float3 c = GetPixel(uv) + GetBlur1(uv)*0.4;
comp_4=`    ret = c * hue_shader;
comp_5=`}
";

fn convert_md2(source: &str) -> milkconv::convert::Converted {
    let file = milkconv::milk::parse(source).expect("the fixture parses");
    milkconv::convert::convert(&file, "md2_fixture").expect("it converts")
}

/// **Both shaders survive the bundle round trip** — translated by the
/// converter, carried in the TOML, validated and loaded back by the engine.
/// This is the seam Phase 4's waves fell through, asserted for the shaders.
#[test]
fn a_milkdrop2_preset_with_both_shaders_converts_and_loads() {
    use lmv_core::preset::Preset;
    use lmv_core::render::scenes::GeneratorConfig;

    let converted = convert_md2(MD2_PRESET);
    let preset = Preset::from_toml_str(&converted.toml)
        .unwrap_or_else(|e| panic!("the emitted bundle must load: {e}\n---\n{}", converted.toml));
    let bundle = match preset.config {
        Some(GeneratorConfig::WarpMesh {
            milk: Some(milk), ..
        }) => *milk,
        other => panic!("expected a bundle, got {other:?}"),
    };
    assert!(bundle.warp_wgsl.is_some(), "the warp shader must survive");
    assert!(bundle.comp_wgsl.is_some(), "the comp shader must survive");
    assert_eq!(bundle.blur_level, 1, "GetBlur1 sets the blur level");
}

/// A bundle whose WGSL does not compile is rejected **by that preset's load**,
/// with naga's report in the error — the loader's per-file skip is what turns
/// this into "loads the rest".
#[test]
fn a_bundle_with_invalid_wgsl_is_rejected_at_load() {
    use lmv_core::preset::Preset;
    let toml = "\
system = \"warp_mesh\"\n\
name   = \"bad shader\"\n\
[milk]\n\
warp_shader = '''\nthis is not wgsl\n'''\n";
    let err = Preset::from_toml_str(toml).expect_err("must be refused");
    let text = err.to_string();
    assert!(
        text.contains("warp_shader"),
        "the error names the shader: {text}"
    );
}

/// **The done-when: a MilkDrop 2 preset with custom warp and composite shaders
/// renders** — and renders *differently* from the same preset stripped of its
/// shaders, so the pipelines demonstrably ran rather than fell back.
///
/// Skipped without an adapter (ADR-0016's policy for captures).
#[test]
fn a_shader_preset_renders_and_its_shaders_have_effect() {
    use lmv_core::dsp::AnalysisFrame;
    use lmv_core::preset::Preset;
    use lmv_core::render::metrics::coverage;
    use lmv_core::render::{HeadlessOptions, Renderer};

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 128,
        height: 96,
        prefer_software: false,
    }) {
        Ok(renderer) => renderer,
        Err(e) => {
            eprintln!("skipping: no adapter ({e})");
            return;
        }
    };
    let frame = AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        waveform: std::array::from_fn(|i| {
            (i as f32 / lmv_core::dsp::WAVE_SAMPLES as f32 * std::f32::consts::TAU * 3.0).sin()
        }),
        ..Default::default()
    };

    let with = Preset::from_toml_str(&convert_md2(MD2_PRESET).toml).expect("loads");
    // The same preset with its shader blocks removed: the built-in warp/present
    // path, same EEL, same waveform.
    let stripped: String = MD2_PRESET
        .lines()
        .filter(|line| !line.starts_with("warp_") && !line.starts_with("comp_"))
        .collect::<Vec<_>>()
        .join("\n");
    let without = Preset::from_toml_str(&convert_md2(&stripped).toml).expect("loads");

    let mut capture = |preset: Preset| {
        let mut preset = preset;
        preset.name = "probe".into();
        renderer.set_presets(vec![preset]);
        renderer
            .capture_preset("probe", &frame, 48)
            .expect("captures")
    };
    let shaded = capture(with);
    let plain = capture(without);

    let lit = coverage(&shaded, [0, 0, 0, 255], 10);
    assert!(
        lit > 0.01,
        "the shader preset must put light on screen (lit = {lit})"
    );
    assert_ne!(
        shaded.rgba, plain.rgba,
        "the translated shaders must change the image, or they never ran"
    );
}
