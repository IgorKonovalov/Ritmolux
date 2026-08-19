//! A parsed `.milk` file → a loadable LMV bundle (Plan 0100 Phase 3).
//!
//! # The roster is the spec
//!
//! MilkDrop's per-frame program writes into a fixed set of named variables and
//! reads from another. This module is that roster, written out, split into what
//! this engine consumes and what it does not — and **an unrecognized name is a
//! named warning, not a silent zero**. A preset that reads a variable MilkDrop
//! supplies and we do not, or writes one MilkDrop draws with and we do not, says
//! so at conversion time in the shape ADR-0020's typo warning already takes.
//!
//! That rule needs one distinction to be usable. In EEL2 an **unset variable
//! legitimately reads zero** — presets use bare names as accumulators constantly,
//! and warning on every one of them would bury the real findings. So the check is
//! against MilkDrop's own roster: a name it supplies and we do not is a warning;
//! a name neither of us has ever heard of is the preset's own scratch and is
//! silent.
//!
//! # What Phase 3 does not convert
//!
//! The waveform, the custom waves and shapes, the borders and the motion vectors
//! ([`UNCONSUMED_OUTPUTS`]) are Phase 4, and the HLSL blocks are Phase 6. A
//! converted preset therefore has **no light source of its own**, because
//! MilkDrop's is the waveform — so the converter emits a **stand-in deposit**,
//! says so in the bundle's header, and Phase 4 replaces it. Without one there
//! would be nothing on screen to judge the motion by, and judging the motion is
//! the phase's whole done-when.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use lmv_core::milk::{ElementKind, MilkBundle, MilkElement};

use crate::eel::{EelError, Symbols, compile_bundle, compile_into};
use crate::milk::MilkFile;

/// A MilkDrop output variable: its `.milk` initial-condition key, its EEL2 name,
/// the value to use when the key is absent, and whether this engine consumes it.
struct Output {
    /// The initial-condition key, lowercased, or `""` where the format has none.
    key: &'static str,
    /// The EEL2 variable a per-frame program writes.
    eel: &'static str,
    /// What MilkDrop uses when the key is absent.
    default: f32,
    /// Whether the engine reads this output.
    consumed: bool,
    /// For an unconsumed output, what would consume it — named in the warning so
    /// a reader knows whether it is coming or never.
    owed: &'static str,
}

/// **The whole output roster**, in the reference's own order.
///
/// Every entry is either consumed by
/// [`warp_mesh`](lmv_core::render::scenes::warp_mesh) or carries the phase that
/// owes it. Nothing is silently dropped: the converter walks this list, seeds
/// each consumed variable from the file's initial condition, and warns by name
/// for each unconsumed one a preset actually writes.
const OUTPUTS: &[Output] = &[
    // --- the warp transform: the nine the mesh drives, plus its exponent ---
    o("zoom", "zoom", 1.0, true, ""),
    o("rot", "rot", 0.0, true, ""),
    o("cx", "cx", 0.5, true, ""),
    o("cy", "cy", 0.5, true, ""),
    o("dx", "dx", 0.0, true, ""),
    o("dy", "dy", 0.0, true, ""),
    o("sx", "sx", 1.0, true, ""),
    o("sy", "sy", 1.0, true, ""),
    o("warp", "warp", 1.0, true, ""),
    // `zoomexp` is consumed indirectly: the converter folds it into the
    // per-vertex program as MilkDrop's own `zoom^(zoomexp^(rad*2-1))`, so the
    // engine needs no parameter for it. See `ZOOMEXP_EPILOGUE`.
    o("fzoomexponent", "zoomexp", 1.0, true, ""),
    // --- the composite roster ---
    o("fdecay", "decay", 0.98, true, ""),
    o("fgammaadj", "gamma", 1.0, true, ""),
    o("btexwrap", "wrap", 1.0, true, ""),
    o("bdarkencenter", "darken_center", 0.0, true, ""),
    o("bbrighten", "brighten", 0.0, true, ""),
    o("bdarken", "darken", 0.0, true, ""),
    o("bsolarize", "solarize", 0.0, true, ""),
    o("binvert", "invert", 0.0, true, ""),
    // --- the video echo, built by Plan 0109 Phase 3 ---
    o("fvideoechozoom", "echo_zoom", 1.0, true, ""),
    o("fvideoechoalpha", "echo_alpha", 0.0, true, ""),
    o("nvideoechoorientation", "echo_orient", 0.0, true, ""),
    // --- the draw layer (Plan 0100 Phase 4) ---
    o("nwavemode", "wave_mode", 0.0, true, ""),
    o("fwavescale", "wave_scale", 1.0, true, ""),
    o("fwavesmoothing", "wave_smoothing", 0.75, true, ""),
    o("fwaveparam", "wave_mystery", 0.0, true, ""),
    o("fwavealpha", "wave_a", 1.0, true, ""),
    o("wave_r", "wave_r", 1.0, true, ""),
    o("wave_g", "wave_g", 1.0, true, ""),
    o("wave_b", "wave_b", 1.0, true, ""),
    o("wave_x", "wave_x", 0.5, true, ""),
    o("wave_y", "wave_y", 0.5, true, ""),
    o("bwavedots", "wave_usedots", 0.0, true, ""),
    o("bwavethick", "wave_thick", 0.0, true, ""),
    // Read but inert: this engine's draw seam is additive by construction
    // (ADR-0056), so a preset asking for an alpha-blended waveform gets an
    // additive one. Consumed rather than warned about because the difference is
    // a blend mode on light that is already premultiplied, not a missing figure.
    o("badditivewaves", "wave_additive", 0.0, true, ""),
    o("bmaximizewavecolor", "wave_brighten", 1.0, true, ""),
    o("ob_size", "ob_size", 0.01, true, ""),
    o("ob_r", "ob_r", 0.0, true, ""),
    o("ob_g", "ob_g", 0.0, true, ""),
    o("ob_b", "ob_b", 0.0, true, ""),
    o("ob_a", "ob_a", 0.0, true, ""),
    o("ib_size", "ib_size", 0.01, true, ""),
    o("ib_r", "ib_r", 0.25, true, ""),
    o("ib_g", "ib_g", 0.25, true, ""),
    o("ib_b", "ib_b", 0.25, true, ""),
    o("ib_a", "ib_a", 0.0, true, ""),
    o("nmotionvectorsx", "mv_x", 12.0, true, ""),
    o("nmotionvectorsy", "mv_y", 9.0, true, ""),
    o("mv_dx", "mv_dx", 0.0, true, ""),
    o("mv_dy", "mv_dy", 0.0, true, ""),
    o("mv_l", "mv_l", 0.9, true, ""),
    o("mv_r", "mv_r", 1.0, true, ""),
    o("mv_g", "mv_g", 1.0, true, ""),
    o("mv_b", "mv_b", 1.0, true, ""),
    o("mv_a", "mv_a", 0.0, true, ""),
    // --- the debug readout, which has no visual effect anywhere ---
    o("", "monitor", 0.0, false, MONITOR),
];

/// Kept for the roster's shape even though nothing carries it today — the draw
/// layer landed in Phase 4 and its entries are consumed. A future unconsumed
/// output names its own phase here.
#[allow(dead_code)]
const DRAW: &str = "Plan 0100 Phase 4, the draw layer";
const MONITOR: &str = "MilkDrop's debug readout, which draws nothing — safe to ignore entirely";

const fn o(
    key: &'static str,
    eel: &'static str,
    default: f32,
    consumed: bool,
    owed: &'static str,
) -> Output {
    Output {
        key,
        eel,
        default,
        consumed,
        owed,
    }
}

/// The read-only variables this engine supplies to a per-frame program.
///
/// Everything MilkDrop supplies **and we do too**. See [`WEAK_INPUTS`] for the
/// ones we supply but not honestly, and [`MISSING_INPUTS`] for the ones we do
/// not.
const SUPPLIED_INPUTS: &[&str] = &[
    "bass", "mid", "treb", "bass_att", "mid_att", "treb_att", "time", "frame", "fps", "meshx",
    "meshy", "aspectx", "aspecty",
];

/// The per-vertex position variables, supplied only inside `per_vertex`.
const VERTEX_INPUTS: &[&str] = &["x", "y", "rad", "ang"];

/// Inputs supplied with a value that is **defined but not what MilkDrop means**,
/// each with the reason. Reading one is a warning, because the preset will render
/// something rather than nothing and the difference is invisible without this.
const WEAK_INPUTS: &[(&str, &str)] = &[(
    "progress",
    "always 0 here: MilkDrop rotates presets on a timer and reports how far \
     through the slot it is, where this engine rotates on a transition and has \
     no slot",
)];

/// Inputs MilkDrop supplies and this engine does not. Reading one gets a zero,
/// and says so.
const MISSING_INPUTS: &[(&str, &str)] = &[
    ("rand_start1", SHADER_RAND),
    ("rand_start2", SHADER_RAND),
    ("rand_start3", SHADER_RAND),
    ("rand_start4", SHADER_RAND),
    ("rand_preset1", SHADER_RAND),
    ("rand_preset2", SHADER_RAND),
    ("rand_preset3", SHADER_RAND),
    ("rand_preset4", SHADER_RAND),
];

const SHADER_RAND: &str = "a MilkDrop 2 random seed. The shader-side pair (`rand_preset`, \
     `rand_frame`) is supplied to translated shaders (Phase 6); this EEL-side \
     quadruple is not, and reads 0";

/// MilkDrop's own `zoomexp`, folded into the per-vertex program.
///
/// The reference computes the per-vertex zoom as
/// `zoom ^ (zoomexp ^ (rad*2 - 1))`, which makes `zoomexp` a *radial* modulation
/// of the zoom rather than an independent output — so it needs no engine
/// parameter at all, only this line appended after the preset's own per-vertex
/// code. Appending it rather than building it into the scene is what keeps the
/// native `warp_mesh` vocabulary free of a knob only converted presets use.
///
/// Guarded: `zoomexp` at exactly 1 is the identity and the common case, and a
/// non-positive one would make `pow` meaningless.
const ZOOMEXP_EPILOGUE: &str = "zoom = if(above(abs(zoomexp - 1), 0.0001) * above(zoomexp, 0), \
     pow(zoom, pow(zoomexp, rad*2 - 1)), zoom);";

/// One thing the converter noticed, in the shape ADR-0020's load warnings take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Which section it came from, or `""` for the file as a whole.
    pub section: &'static str,
    /// The message, one line.
    pub message: String,
    /// A short machine-readable class, for Phase 5's failure ranking.
    pub class: &'static str,
}

/// A converted preset: the bundle, the preset text that carries it, and
/// everything the converter noticed on the way.
pub struct Converted {
    /// The whole `.toml` a bundle is, ready to write.
    pub toml: String,
    /// The compiled programs, for a caller that wants to run them without a
    /// round trip through text.
    pub bundle: MilkBundle,
    /// Named findings — never silent, per the phase's rule.
    pub warnings: Vec<Warning>,
}

/// Why a `.milk` file did not convert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// A section did not compile.
    Eel(EelError),
    /// A MilkDrop 2 shader did not translate (Plan 0100 Phase 6). The whole
    /// preset is rejected **by name** rather than converted without its shader:
    /// a preset whose picture lives in its `warp` block would otherwise load
    /// without complaint and render something its author never drew, which is
    /// the failure class the plan's Risks call the worst for reputation.
    Shader {
        /// `"warp"` or `"comp"`.
        stage: &'static str,
        /// What did not translate — class first, so the report ranks by it.
        err: crate::shader::ShaderError,
    },
    /// The translator emitted WGSL that naga then refused — a converter defect
    /// by definition, and ranked as its own class so it can never hide inside a
    /// preset-shaped reason.
    EmitterInvalid {
        /// `"warp"` or `"comp"`.
        stage: &'static str,
        /// naga's report.
        err: String,
    },
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Eel(e) => write!(f, "{e}"),
            // The first colon-free segment is the ranking key (`reason_class`),
            // so the stage and the class both sit before the first `:`.
            ConvertError::Shader { stage, err } => {
                write!(f, "{stage} shader {}: {}", err.class, err.message)
            }
            ConvertError::EmitterInvalid { stage, err } => {
                write!(
                    f,
                    "{stage} shader emitter-invalid (converter defect): {err}"
                )
            }
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<EelError> for ConvertError {
    fn from(e: EelError) -> Self {
        ConvertError::Eel(e)
    }
}

/// Convert a parsed `.milk` file into a bundle and the preset text carrying it.
///
/// `name` is what the preset will be called — the caller's file stem, since the
/// format has no name field of its own.
pub fn convert(file: &MilkFile, name: &str) -> Result<Converted, ConvertError> {
    let mut warnings = Vec::new();

    // MilkDrop 1 calls it `per_pixel` and MilkDrop 2 `per_vertex`, and a file may
    // carry both — the reference concatenates them, so this does too.
    let per_vertex_src = match (file.block("per_pixel"), file.block("per_vertex")) {
        ("", v) => v.to_string(),
        (p, "") => p.to_string(),
        (p, v) => format!("{p}\n{v}"),
    };

    // **The initial conditions run at the top of every frame**, which is
    // MilkDrop's own semantics: the output variables are reset to the preset's
    // declared values before its per-frame code runs, while `q1`-`q32` and the
    // preset's own variables persist. Emitting them as a prologue rather than as
    // engine state is what makes that true without a second mechanism.
    let mut prologue = String::new();
    for out in OUTPUTS {
        if out.key.is_empty() {
            continue;
        }
        let value = file.number(out.key, out.default);
        let _ = writeln!(prologue, "{} = {value:?};", out.eel);
    }

    let per_frame_src = format!("{prologue}\n{}", file.block("per_frame"));
    // `zoomexp` last, so it modulates whatever the preset's own code left in
    // `zoom` rather than being overwritten by it.
    // The separating `;` is not decoration: the corpus contains blocks whose last
    // statement has no terminator, and without it the epilogue would run on into
    // the preset's own last line and fail to compile.
    let per_vertex_src = format!("{per_vertex_src};\n{ZOOMEXP_EPILOGUE}");

    let (mut bundle, _) = compile_bundle(
        file.block("per_frame_init"),
        &per_frame_src,
        &per_vertex_src,
    )?;
    let (waves, shapes) = build_elements(file, &mut warnings)?;
    bundle.waves = waves;
    bundle.shapes = shapes;

    // --- the shaders (Plan 0100 Phase 6) ---
    //
    // Only a MilkDrop 2 preset runs its shader blocks; a 1.x file carrying
    // stray `warp_N` lines (the corpus has a few, from hand editing) never
    // executed them in the reference either.
    let mut shader_stats = Vec::new();
    if file.is_milkdrop2() {
        let tex_wrap = file.number("btexwrap", 1.0) >= 0.5;
        for (stage, block) in [
            (crate::shader::Stage::Warp, "warp"),
            (crate::shader::Stage::Comp, "comp"),
        ] {
            let source = file.block(block);
            if source.trim().is_empty() {
                continue;
            }
            let translated = crate::shader::translate(stage, source, tex_wrap).map_err(|err| {
                ConvertError::Shader {
                    stage: stage.name(),
                    err,
                }
            })?;
            // The same gate the loader runs, run now — an emitter bug is a
            // converter finding, not a load-time mystery.
            lmv_core::milk::shader::validate_wgsl(&translated.wgsl).map_err(|err| {
                ConvertError::EmitterInvalid {
                    stage: stage.name(),
                    err,
                }
            })?;
            shader_stats.push((stage.name(), translated.ops, translated.loops));
            bundle.blur_level = bundle.blur_level.max(translated.blur_level);
            match stage {
                crate::shader::Stage::Warp => bundle.warp_wgsl = Some(translated.wgsl),
                crate::shader::Stage::Comp => bundle.comp_wgsl = Some(translated.wgsl),
            }
        }
    }

    // --- the roster check: nothing unrecognized goes through silently ---
    //
    // **Against the PRESET's own code, not the generated prologue.** The
    // prologue seeds every output the format declares an initial condition for,
    // including the ones this engine does not consume — that is correct, because
    // a preset may read `wave_r` to compute something that *is* consumed — but it
    // means the compiled bundle's symbol table mentions names the preset never
    // wrote. Checking against that table would warn about every unconsumed output
    // on every preset, and a warning that always fires is one nobody reads.
    //
    // So the preset's own blocks are compiled a second time into a throwaway
    // table. It costs one extra compile of text already in memory, and it is what
    // makes each finding below a fact about the preset.
    let (user_written, user_read) = user_symbols(file, &per_vertex_src);

    for out in OUTPUTS {
        if out.consumed || !user_written.contains(out.eel) {
            continue;
        }
        warnings.push(Warning {
            section: "",
            message: format!(
                "sets `{}`, which this engine does not consume — {}",
                out.eel, out.owed
            ),
            class: if out.owed == DRAW {
                "unconsumed-draw"
            } else {
                "unconsumed-other"
            },
        });
    }

    for (input, why) in WEAK_INPUTS {
        if user_read.contains(*input) {
            warnings.push(Warning {
                section: "",
                message: format!("reads `{input}`, which is {why}"),
                class: "weak-input",
            });
        }
    }
    for (input, why) in MISSING_INPUTS {
        if user_read.contains(*input) {
            warnings.push(Warning {
                section: "",
                message: format!(
                    "reads `{input}`, which this engine does not supply (it reads 0) — {why}"
                ),
                class: "missing-input",
            });
        }
    }
    // A preset that WRITES a host input is overwriting a value the engine
    // rewrites at the top of the next frame — legal EEL2, and almost always a
    // preset meaning to use a variable of its own.
    for input in SUPPLIED_INPUTS {
        if user_written.contains(*input) {
            warnings.push(Warning {
                section: "",
                message: format!(
                    "writes `{input}`, which the host supplies and overwrites every                      frame — the assignment survives only until the next frame"
                ),
                class: "writes-input",
            });
        }
    }
    // ...and one that reads a per-VERTEX position outside `per_vertex` reads
    // whatever the last vertex left, which is a real MilkDrop trap rather than a
    // difference here.
    let frame_only = user_frame_symbols(file);
    for input in VERTEX_INPUTS {
        if frame_only.contains(*input) {
            warnings.push(Warning {
                section: "per_frame",
                message: format!(
                    "reads `{input}` in per-frame code, where it is a per-VERTEX                      variable — it holds whatever the previous frame's last vertex                      left it at"
                ),
                class: "vertex-input-in-frame",
            });
        }
    }

    // A 1.x file carrying stray shader text: the reference never ran it, so the
    // conversion does not either — but it is said, not silent.
    if !file.is_milkdrop2()
        && (!file.block("warp").trim().is_empty() || !file.block("comp").trim().is_empty())
    {
        warnings.push(Warning {
            section: "",
            message: "carries shader text but declares MilkDrop 1.x, which never ran it — \
                      ignored, as the reference ignored it"
                .into(),
            class: "shader-on-milkdrop1",
        });
    }
    if file.key("sampler_pc").is_some() || uses_disk_texture(file) {
        warnings.push(Warning {
            section: "",
            message: "samples a disk texture, which is deliberately out of scope — the \
                      preset will render without it"
                .into(),
            class: "disk-texture",
        });
    }

    let toml = emit(
        file,
        name,
        &bundle,
        &warnings,
        &per_frame_src,
        &per_vertex_src,
        &shader_stats,
    );
    Ok(Converted {
        toml,
        bundle,
        warnings,
    })
}

/// The names the **preset's own** code writes and reads, ignoring the generated
/// prologue and epilogue — see the roster check for why the distinction matters.
///
/// A compile failure here is impossible in practice (the same text compiled a
/// moment ago as part of the bundle) and is treated as "nothing referenced"
/// rather than propagated: this is the *reporting* path, and a converted preset
/// should not fail over its own diagnostics.
fn user_symbols(file: &MilkFile, per_vertex_src: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    // The epilogue and the `;` that separates it are ours, not the preset's, so
    // they are trimmed back off before the referenced names are collected.
    let vertex = per_vertex_src
        .strip_suffix(ZOOMEXP_EPILOGUE)
        .and_then(|v| v.strip_suffix(";\n"))
        .unwrap_or(per_vertex_src)
        .to_string();
    let Ok((bundle, symbols)) = compile_bundle(
        file.block("per_frame_init"),
        file.block("per_frame"),
        &vertex,
    ) else {
        return (BTreeSet::new(), BTreeSet::new());
    };
    let mut written = BTreeSet::new();
    for program in [
        &bundle.per_frame_init,
        &bundle.per_frame,
        &bundle.per_vertex,
    ] {
        for index in program.written_registers() {
            if let Some(name) = program.names().get(*index as usize) {
                written.insert(name.clone());
            }
        }
    }
    // Everything referenced but never written is read-only from the preset's
    // point of view, which is exactly the set the input warnings ask about.
    let read = symbols
        .names()
        .iter()
        .filter(|name| !written.contains(*name))
        .cloned()
        .collect();
    (written, read)
}

/// The names the preset's **per-frame** code alone references — the narrower
/// question the per-vertex-input warning needs.
fn user_frame_symbols(file: &MilkFile) -> BTreeSet<String> {
    let mut symbols = Symbols::new();
    if compile_into(file.block("per_frame"), &mut symbols).is_err() {
        return BTreeSet::new();
    }
    symbols.names().iter().cloned().collect()
}

/// Whether the file's shader blocks name a user texture — the exclusion Plan
/// 0100 states and prices at 19 % of the corpus.
fn uses_disk_texture(file: &MilkFile) -> bool {
    let text = format!("{}\n{}", file.block("warp"), file.block("comp"));
    // MilkDrop declares one as `sampler_<name>` for any name that is not one of
    // its built-ins.
    text.split("sampler_").skip(1).any(|rest| {
        let word: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        !matches!(
            word.as_str(),
            "main"
                | "fw_main"
                | "fc_main"
                | "pw_main"
                | "pc_main"
                | "blur1"
                | "blur2"
                | "blur3"
                | "noise_lq"
                | "noise_lq_lite"
                | "noise_mq"
                | "noise_hq"
                | "noisevol_lq"
                | "noisevol_hq"
        )
    })
}

/// The palette a converted preset carries.
///
/// **The deposit is gone as of Phase 4**: a converted preset draws its own light
/// now — the waveform, its custom waves and shapes, the borders — so the
/// stand-in ring Phase 3 emitted would be a second, invented figure on top of the
/// preset's real one. The scene turns the deposit off for the whole of a bundle's
/// life (`WarpMeshScene::update`), and this emits none.
///
/// What survives is the palette, and it is **not** what colours the draw layer:
/// every stroke takes its colour from the preset's own `wave_r`/`_g`/`_b` and its
/// elements' own. The palette is here because the scene's colour vocabulary is
/// shared with every other system and a preset must carry one; a converted preset
/// simply never samples it.
fn deposit_block(file: &MilkFile) -> String {
    // The waveform's colour, which is the closest thing the format has to "what
    // colour is this preset" before its per-frame code runs.
    let (r, g, b) = (
        file.number("wave_r", 1.0).clamp(0.0, 1.0),
        file.number("wave_g", 1.0).clamp(0.0, 1.0),
        file.number("wave_b", 1.0).clamp(0.0, 1.0),
    );
    let hex = |s: f32| -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * s * 255.0) as u8,
            (g * s * 255.0) as u8,
            (b * s * 255.0) as u8
        )
    };
    format!(
        "[palette]\n\
         stops = [\n\
         \x20 {{ at = 0.0,  color = \"#000000\" }},\n\
         \x20 {{ at = 0.45, color = \"{}\" }},\n\
         \x20 {{ at = 0.8,  color = \"{}\" }},\n\
         \x20 {{ at = 1.0,  color = \"#ffffff\" }},\n\
         ]\n\
         \n\
         [params]\n\
         # A converted preset draws its own light - the waveform, its custom\n\
         # waves and shapes, the borders - so the scene's own deposit stays off.\n\
         brightness     = \"1.0\"\n",
        hex(0.55),
        hex(1.0),
    )
}

/// Assemble the whole preset text.
fn emit(
    file: &MilkFile,
    name: &str,
    bundle: &MilkBundle,
    warnings: &[Warning],
    per_frame_src: &str,
    per_vertex_src: &str,
    shader_stats: &[(&'static str, u32, u32)],
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Converted from a MilkDrop preset by `milkconv` (Plan 0100 / ADR-0113).\n\
         #\n\
         # Everything below is machine-generated. The `[milk]` table is compiled\n\
         # EEL2 bytecode, executed by the engine's stack VM; the EEL2 it came from\n\
         # is reproduced under each section so this file can be read.\n\
         #\n\
         # Source format: MilkDrop {}",
        if file.is_milkdrop2() { "2" } else { "1.x" }
    );
    if warnings.is_empty() {
        let _ = writeln!(out, "# The converter had nothing to report.");
    } else {
        let _ = writeln!(out, "#\n# WHAT THE CONVERTER COULD NOT CARRY ACROSS:");
        for warning in warnings {
            let _ = writeln!(out, "#   - {}", warning.message);
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "system = \"warp_mesh\"");
    let _ = writeln!(out, "name   = \"{}\"", name.replace('"', "'"));
    let _ = writeln!(out);
    // The grid MilkDrop itself would use, clamped by the tier at load.
    let _ = writeln!(out, "[mesh]");
    let _ = writeln!(
        out,
        "x = {}",
        (file.number("nmeshx", 32.0) as u32).clamp(2, 128)
    );
    let _ = writeln!(
        out,
        "y = {}",
        (file.number("nmeshy", 24.0) as u32).clamp(2, 96)
    );
    let _ = writeln!(out);
    out.push_str(&deposit_block(file));
    // `fWarpScale` and `fWarpAnimSpeed` are preset-level in the format rather
    // than per-frame outputs, so they land as ordinary bindings.
    let _ = writeln!(
        out,
        "warp_scale     = \"{:?}\"",
        file.number("fwarpscale", 1.0)
    );
    let _ = writeln!(
        out,
        "warp_speed     = \"{:?}\"",
        file.number("fwarpanimspeed", 1.0)
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "[milk]");
    for (section, source, program) in [
        (
            "per_frame_init",
            file.block("per_frame_init"),
            &bundle.per_frame_init,
        ),
        ("per_frame", per_frame_src, &bundle.per_frame),
        ("per_vertex", per_vertex_src, &bundle.per_vertex),
    ] {
        let _ = writeln!(out, "# --- {section}, from:");
        for line in source.lines() {
            let _ = writeln!(out, "#   {}", line.trim_end());
        }
        let _ = writeln!(out, "{section} = \"\"\"\n{}\"\"\"\n", program.to_assembly());
    }
    // The translated shaders (Plan 0100 Phase 6) — before the array-of-tables,
    // which close the `[milk]` table. TOML *literal* strings, because WGSL never
    // contains a quote and a basic string would reinterpret its backslashes (of
    // which WGSL has none today, and this keeps it that way).
    for (key, wgsl) in [
        ("warp_shader", &bundle.warp_wgsl),
        ("comp_shader", &bundle.comp_wgsl),
    ] {
        if let Some(wgsl) = wgsl {
            let stats = shader_stats
                .iter()
                .find(|(stage, _, _)| key.starts_with(stage));
            if let Some((stage, ops, loops)) = stats {
                let _ = writeln!(
                    out,
                    "# --- the {stage} shader, translated from HLSL: {ops} ops, \
                     {loops} loop(s), each bounded at 1024 iterations ---"
                );
            }
            let _ = writeln!(out, "{key} = '''\n{wgsl}'''\n");
        }
    }
    if bundle.blur_level > 0 {
        let _ = writeln!(out, "blur_level = {}\n", bundle.blur_level);
    }
    emit_elements(&mut out, "waves", &bundle.waves);
    emit_elements(&mut out, "shapes", &bundle.shapes);
    out
}

/// The `[[milk.waves]]` / `[[milk.shapes]]` array-of-tables a bundle's custom
/// elements load back from.
///
/// **Emitted after the three `[milk]` keys and never before them**: TOML array-of
/// tables close the table they are nested in, so a `per_frame` written after the
/// first `[[milk.waves]]` would parse as a key of *that* wave.
fn emit_elements(out: &mut String, which: &str, elements: &[MilkElement]) {
    for element in elements {
        let _ = writeln!(out, "[[milk.{which}]]");
        let _ = writeln!(out, "count = {}", element.count);
        if element.instances != 1 {
            let _ = writeln!(out, "instances = {}", element.instances);
        }
        if element.use_dots {
            let _ = writeln!(out, "use_dots = true");
        }
        if element.thick {
            let _ = writeln!(out, "thick = true");
        }
        if element.additive {
            let _ = writeln!(out, "additive = true");
        }
        for (key, program) in [
            ("init", &element.init),
            ("per_frame", &element.per_frame),
            ("per_point", &element.per_point),
        ] {
            // An element with no per-point code — every shape — writes no key
            // rather than an empty string, which is what the loader's `Option`
            // already means.
            if program.register_count() == 0 && program.to_assembly().trim().is_empty() {
                continue;
            }
            let _ = writeln!(out, "{key} = \"\"\"\n{}\"\"\"", program.to_assembly());
        }
        let _ = writeln!(out);
    }
}

// ---------------------------------------------------------------------------
// The custom waves and shapes (Plan 0100 Phase 4)
// ---------------------------------------------------------------------------

/// A custom **wave**'s initial-condition keys, as `(suffix, eel name, default)`.
///
/// The suffix is what follows `wavecode_N_`. Everything here is seeded into the
/// element's own register file by a prologue, exactly as the main bundle's
/// initial conditions are — so an element's per-frame code starts from what the
/// file says and the engine needs no struct of `.milk` keys.
const WAVE_KEYS: &[(&str, &str, f32)] = &[
    ("r", "r", 1.0),
    ("g", "g", 1.0),
    ("b", "b", 1.0),
    ("a", "a", 1.0),
    ("x", "x", 0.5),
    ("y", "y", 0.5),
    ("scaling", "scaling", 1.0),
    ("smoothing", "smoothing", 0.5),
    ("sep", "sep", 0.0),
];

/// A custom **shape**'s initial-condition keys — see [`WAVE_KEYS`].
const SHAPE_KEYS: &[(&str, &str, f32)] = &[
    ("x", "x", 0.5),
    ("y", "y", 0.5),
    ("rad", "rad", 0.1),
    ("ang", "ang", 0.0),
    ("r", "r", 1.0),
    ("g", "g", 0.0),
    ("b", "b", 0.0),
    ("a", "a", 1.0),
    ("r2", "r2", 0.0),
    ("g2", "g2", 1.0),
    ("b2", "b2", 0.0),
    ("a2", "a2", 0.0),
    ("border_r", "border_r", 1.0),
    ("border_g", "border_g", 1.0),
    ("border_b", "border_b", 1.0),
    ("border_a", "border_a", 0.1),
    ("thickoutline", "thickoutline", 0.0),
    ("sides", "sides", 4.0),
    ("textured", "textured", 0.0),
    ("tex_ang", "tex_ang", 0.0),
    ("tex_zoom", "tex_zoom", 1.0),
    ("additive", "additive", 0.0),
];

/// Compile the file's up-to-four custom waves and up-to-four custom shapes.
///
/// A disabled element is skipped entirely — it costs no register file, no
/// programs and no geometry — which is what keeps the common case (a preset with
/// one enabled wave out of four declared) from paying for the other three.
fn build_elements(
    file: &MilkFile,
    warnings: &mut Vec<Warning>,
) -> Result<(Vec<MilkElement>, Vec<MilkElement>), ConvertError> {
    let mut waves = Vec::new();
    let mut shapes = Vec::new();

    for n in 0..4u32 {
        if file.number(&format!("wavecode_{n}_enabled"), 0.0) < 0.5 {
            continue;
        }
        let prologue = element_prologue(file, &format!("wavecode_{n}_"), WAVE_KEYS);
        let element = compile_element(
            file,
            &format!("wave_{n}"),
            &prologue,
            ElementKind::Wave,
            file.number(&format!("wavecode_{n}_samples"), 512.0) as u32,
            1,
            file.number(&format!("wavecode_{n}_busedots"), 0.0) >= 0.5,
            file.number(&format!("wavecode_{n}_bdrawthick"), 0.0) >= 0.5,
            file.number(&format!("wavecode_{n}_badditive"), 0.0) >= 0.5,
        )?;
        if file.number(&format!("wavecode_{n}_bspectrum"), 0.0) >= 0.5 {
            warnings.push(Warning {
                section: "",
                message: format!(
                    "custom wave {n} asks for the SPECTRUM as its source rather than the \
                     waveform; it is drawn from the waveform here, so its figure is the \
                     right shape over the wrong signal"
                ),
                class: "wave-spectrum-source",
            });
        }
        waves.push(element);
    }

    for n in 0..4u32 {
        if file.number(&format!("shapecode_{n}_enabled"), 0.0) < 0.5 {
            continue;
        }
        let prologue = element_prologue(file, &format!("shapecode_{n}_"), SHAPE_KEYS);
        let element = compile_element(
            file,
            &format!("shape_{n}"),
            &prologue,
            ElementKind::Shape,
            file.number(&format!("shapecode_{n}_sides"), 4.0) as u32,
            file.number(&format!("shapecode_{n}_num_inst"), 1.0) as u32,
            false,
            file.number(&format!("shapecode_{n}_thickoutline"), 0.0) >= 0.5,
            // A shape's own `additive` is seeded into its register file by
            // `SHAPE_KEYS` and may be rewritten per instance, so the element-level
            // flag is unused for shapes — see `ElementSpec::additive`.
            false,
        )?;
        if file.number(&format!("shapecode_{n}_textured"), 0.0) >= 0.5 {
            warnings.push(Warning {
                section: "",
                message: format!(
                    "custom shape {n} is textured with the previous frame, which this \
                     engine has no stage for; it is drawn as a flat gradient fill"
                ),
                class: "shape-textured",
            });
        }
        shapes.push(element);
    }

    Ok((waves, shapes))
}

/// The initial-condition assignments for one element, as EEL2 — prepended to its
/// per-frame program so they are re-applied every frame, which is MilkDrop's own
/// semantics for the main program and for these alike.
fn element_prologue(file: &MilkFile, prefix: &str, keys: &[(&str, &str, f32)]) -> String {
    let mut out = String::new();
    for (suffix, eel, default) in keys {
        let value = file.number(&format!("{prefix}{suffix}"), *default);
        let _ = writeln!(out, "{eel} = {value:?};");
    }
    out
}

/// Compile one element's three programs against **one shared register table of
/// its own**.
///
/// Its own, not the main bundle's: MilkDrop gives each custom wave and shape a
/// separate variable scope, and only `q1`-`q32` cross (by copy, see
/// `milk::Q_COUNT`). Sharing would let one element's `t1` collide with another's,
/// which real presets rely on not happening.
#[allow(clippy::too_many_arguments)]
fn compile_element(
    file: &MilkFile,
    block: &str,
    prologue: &str,
    kind: ElementKind,
    count: u32,
    instances: u32,
    use_dots: bool,
    thick: bool,
    additive: bool,
) -> Result<MilkElement, ConvertError> {
    let per_frame = format!("{prologue}\n{};", file.block(&format!("{block}_per_frame")));
    let per_point = file.block(&format!("{block}_per_point")).to_string();
    let (bundle, _) = compile_bundle(file.block(&format!("{block}_init")), &per_frame, &per_point)?;
    Ok(MilkElement {
        init: bundle.per_frame_init,
        per_frame: bundle.per_frame,
        per_point: bundle.per_vertex,
        count,
        instances,
        kind,
        use_dots,
        thick,
        additive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The four roster tables partition the names they describe.** They are
    /// hand-written lists, and a name that landed in two of them would either
    /// warn about something the engine supplies or stay silent about something it
    /// does not — the two errors this whole module exists to prevent.
    #[test]
    fn the_roster_tables_do_not_overlap() {
        let supplied: BTreeSet<&str> = SUPPLIED_INPUTS
            .iter()
            .chain(VERTEX_INPUTS)
            .copied()
            .collect();
        for (name, _) in WEAK_INPUTS.iter().chain(MISSING_INPUTS) {
            assert!(
                !supplied.contains(name),
                "`{name}` is listed both as supplied and as weak/missing"
            );
        }
        for out in OUTPUTS {
            assert!(
                !supplied.contains(out.eel),
                "`{}` is both an output and a supplied input",
                out.eel
            );
        }
        // Every output name is distinct, and so is every initial-condition key.
        let mut eel = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for out in OUTPUTS {
            assert!(eel.insert(out.eel), "`{}` appears twice", out.eel);
            assert!(
                out.key.is_empty() || keys.insert(out.key),
                "key `{}` appears twice",
                out.key
            );
        }
    }

    /// **Every output the engine consumes actually reaches it** — the seam
    /// between this table and the engine's, checked rather than assumed.
    ///
    /// There are two ways in, and both are checked: a **scene parameter** the
    /// warp mesh declares, or a field on `milk::outputs::FrameOutputs`, which the
    /// scene reads directly for the composite roster and the whole draw layer. A
    /// roster entry marked consumed with neither is a name this converter seeds
    /// into a bundle that nothing will ever look at, which is precisely the
    /// silent loss the no-silent-zero rule is about.
    ///
    /// `zoomexp` is the one exception and is folded into the per-vertex program
    /// instead ([`ZOOMEXP_EPILOGUE`]).
    #[test]
    fn every_consumed_output_reaches_the_engine() {
        use lmv_core::milk::outputs::FRAME_OUTPUT_NAMES;
        use lmv_core::render::scenes::warp_mesh::PARAMS;
        for out in OUTPUTS {
            if !out.consumed {
                continue;
            }
            if out.eel == "zoomexp" {
                assert!(
                    ZOOMEXP_EPILOGUE.contains("zoomexp"),
                    "`zoomexp` is consumed by the per-vertex epilogue, which must \
                     name it"
                );
                continue;
            }
            let known = PARAMS.contains(&out.eel) || FRAME_OUTPUT_NAMES.contains(&out.eel);
            assert!(
                known,
                "the roster says `{}` is consumed, but neither the warp mesh's \
                 params nor `FrameOutputs` has it — either wire it up or mark it \
                 unconsumed with the phase that owes it",
                out.eel
            );
        }
    }
}
