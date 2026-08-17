//! The `.milk` file format: sections, keys, and the numbered code blocks.
//!
//! A `.milk` file is an INI-shaped list of `key=value` lines under a single
//! `[preset00]` header. Most keys are scalars — the preset's initial conditions
//! and its flags — but the **program text is split across numbered keys**:
//!
//! ```text
//! per_frame_1=wave_r = wave_r + 0.25*sin(1.4*time);
//! per_frame_2=wave_g = wave_g + 0.25*sin(1.7*time);
//! ```
//!
//! Those are one program, joined in index order. MilkDrop's writer chops at a
//! fixed width without regard for token boundaries, so a line may be cut
//! **mid-identifier** — which decides how the join has to work; see
//! [`join_code`], where the choice is measured rather than assumed.
//!
//! # What this module does not do
//!
//! It does not know what any key means. Splitting the file into `(scalars,
//! blocks)` is a lexical job with its own failure modes — an index that does not
//! parse, a key with no `=`, a file with no recognizable content — and keeping it
//! separate from [`crate::convert`]'s roster is what lets the roster be a table
//! rather than a parser.

use std::collections::BTreeMap;

/// The code blocks a `.milk` file can carry, by the prefix their numbered keys
/// use.
///
/// An allowlist rather than "any key ending in `_<digits>`", deliberately: a
/// scalar key that happened to end in a number would otherwise be swallowed into
/// a program and vanish from the roster check, which is the one direction of
/// error this parser must not make silently.
///
/// `per_pixel` is MilkDrop 1's name for what MilkDrop 2 calls `per_vertex`, and
/// both appear in the corpus — often in the same file, which is why they are
/// collected separately and merged by the converter rather than here.
const BLOCK_PREFIXES: &[&str] = &[
    "per_frame_init",
    "per_frame",
    "per_pixel",
    "per_vertex",
    // MilkDrop 2's HLSL blocks. Collected here so Phase 5's report can say a
    // preset has them; translated in Phase 6.
    "warp",
    "comp",
];

/// One parsed `.milk` file.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MilkFile {
    /// Every scalar `key=value`, key **lowercased** (the format is
    /// case-insensitive and the corpus is inconsistent — `bTexWrap` and
    /// `btexwrap` both appear).
    pub keys: BTreeMap<String, String>,
    /// Each code block's source, joined in index order. Keyed by
    /// the prefix, including the per-wave and per-shape ones
    /// (`wave_0_per_point`, `shape_1_per_frame`).
    pub blocks: BTreeMap<String, String>,
}

/// Why a `.milk` file did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MilkError {
    /// The file holds no `key=value` line at all — it is not a preset.
    NotAPreset,
}

impl std::fmt::Display for MilkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MilkError::NotAPreset => write!(
                f,
                "no `key=value` line found — this is not a MilkDrop preset"
            ),
        }
    }
}

impl std::error::Error for MilkError {}

impl MilkFile {
    /// A scalar key's value, or `None`.
    pub fn key(&self, name: &str) -> Option<&str> {
        self.keys
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// A scalar key as a number, or `default` when absent or unparseable.
    ///
    /// Unparseable rather than an error on purpose: the corpus contains keys
    /// written as `1.#QNAN` and `-1.#INF` (a Windows `printf` of a value that
    /// had already gone wrong), and refusing a whole preset over one of them
    /// would lose a preset that renders fine.
    pub fn number(&self, name: &str, default: f32) -> f32 {
        self.key(name)
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(default)
    }

    /// A code block's joined source, or `""`.
    pub fn block(&self, name: &str) -> &str {
        self.blocks.get(name).map_or("", String::as_str)
    }

    /// Whether the file declares itself a MilkDrop 2 preset, i.e. whether it can
    /// carry HLSL. **82 % of the corpus does** (8 500 of 10 347, measured before
    /// the plan started), which is what Phase 6's stop condition is priced
    /// against.
    pub fn is_milkdrop2(&self) -> bool {
        self.key("milkdrop_preset_version")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .is_some_and(|v| v >= 200.0)
    }
}

/// Parse `.milk` text into scalars and code blocks.
///
/// The input is read as **bytes** by the caller and converted lossily: these
/// files are Windows-era text, mostly ASCII but with the occasional Latin-1 byte
/// in a preset name, and a strict UTF-8 read would reject a preset over its
/// title.
pub fn parse(text: &str) -> Result<MilkFile, MilkError> {
    let mut out = MilkFile::default();
    // Indexed lines, per block, so they can be joined in numeric order however
    // the file happened to order them.
    let mut indexed: BTreeMap<String, BTreeMap<u32, String>> = BTreeMap::new();

    for raw in text.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('[') {
            continue;
        }
        // Split at the FIRST `=`: a code line's value is full of them.
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        match split_indexed(&key) {
            Some((prefix, index)) => {
                indexed
                    .entry(prefix)
                    .or_default()
                    // A duplicate index keeps the LAST, which is what a
                    // hand-edited file with a repeated line means.
                    .insert(index, value.to_string());
            }
            None => {
                out.keys.insert(key, value.trim().to_string());
            }
        }
    }

    if out.keys.is_empty() && indexed.is_empty() {
        return Err(MilkError::NotAPreset);
    }

    for (prefix, lines) in indexed {
        let joined = if matches!(prefix.as_str(), "warp" | "comp") {
            join_shader(lines.into_values())
        } else {
            join_code(lines.into_values())
        };
        out.blocks.insert(prefix, joined);
    }
    Ok(out)
}

/// Join a **shader** block's numbered lines.
///
/// Shader lines are the one part of the format MilkDrop's writer does *not*
/// chop: each stored line is one source line, written verbatim behind a leading
/// backtick (the format's way of protecting leading whitespace from INI
/// trimming). So the correct join is the opposite of [`join_code`]'s — strip
/// the backtick, keep the newline, and leave `//` comments for the shader lexer,
/// which knows where a `#define` ends only because the line does.
fn join_shader(lines: impl Iterator<Item = String>) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line.strip_prefix('`').unwrap_or(&line));
        out.push('\n');
    }
    out
}

/// Join a code block's numbered lines into one program.
///
/// # The join is direct, and that is measured rather than assumed
///
/// **MilkDrop's own writer chops a program at a fixed width**, without regard for
/// token boundaries, so the corpus contains lines cut mid-identifier:
///
/// ```text
/// per_frame_22=k1 =  is_
/// per_frame_23=beat*equal(index%2,0);
/// ```
///
/// That is `is_beat`, and it is only `is_beat` if the lines are concatenated with
/// **nothing between them**. Joining with a newline — which is what projectM and
/// Butterchurn both do — turns it into two identifiers with no operator, and the
/// preset fails to compile. Twenty-odd files in the 552-preset original pack are
/// cut that way, all by the same authors; measured on that pack, the direct join
/// converts **552 of 552** where the newline join converts 525.
///
/// The reason a newline join is tempting is the other half of the format: blocks
/// also contain `//` line comments, and concatenating those directly would
/// comment out **the rest of the program** — silently, producing a preset that
/// loads and renders almost nothing. So the comments are stripped here, per
/// source line, *before* the join. That is the combination that is correct for
/// both: a mid-token cut rejoins, and a comment ends where its line ended.
///
/// A `/* */` block comment spanning several lines still works, because the lexer
/// sees it after the join and does not care where the line breaks were.
fn join_code(lines: impl Iterator<Item = String>) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(strip_line_comment(&line));
    }
    out
}

/// `line` up to its first `//`, which in EEL2 runs to the end of the line.
///
/// `/*` is deliberately left alone: it is a *block* comment and may legitimately
/// span several stored lines, so the lexer is the right place for it.
fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(at) => line.get(..at).unwrap_or(""),
        None => line,
    }
}

/// `("per_frame", 3)` for `per_frame_3`, or `None` for a scalar key.
///
/// **The format spells its index two ways, and both are in the corpus.** The
/// main blocks separate it — `per_frame_3`, `per_frame_init_12`, `warp_5` — while
/// the per-wave and per-shape blocks append it directly: `wave_0_per_point7`,
/// `shape_2_per_frame3`. Reading only the first form loses every custom wave and
/// shape *silently*, as scalar keys nothing consumes, which is exactly the kind
/// of quiet loss the roster check exists to prevent.
///
/// The prefix must be a known block name either way — see [`BLOCK_PREFIXES`] for
/// why an allowlist rather than a shape test.
fn split_indexed(key: &str) -> Option<(String, u32)> {
    // The separated form: everything up to the last `_`, then digits.
    if let Some((prefix, suffix)) = key.rsplit_once('_')
        && let Ok(index) = suffix.parse::<u32>()
        && BLOCK_PREFIXES.contains(&prefix)
    {
        return Some((prefix.to_string(), index));
    }
    // The appended form: a trailing run of digits with no separator.
    let digits = key.len() - key.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let split = key.len() - digits;
    let prefix = key.get(..split)?;
    let index = key.get(split..)?.parse::<u32>().ok()?;
    is_element_block(prefix).then(|| (prefix.to_string(), index))
}

/// Whether `prefix` is a custom wave's or shape's code block —
/// `wave_0_per_point`, `shape_2_per_frame`, `wave_1_init`.
///
/// Collected in Phase 3 so Phase 5's report can count them; consumed in Phase 4.
fn is_element_block(prefix: &str) -> bool {
    let mut parts = prefix.splitn(3, '_');
    let kind = parts.next().unwrap_or("");
    let index = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    matches!(kind, "wave" | "shape")
        && index.parse::<u32>().is_ok()
        && matches!(rest, "init" | "per_frame" | "per_point")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The format's own shape: a header, scalars, and numbered code lines joined
    /// in index order.
    #[test]
    fn a_preset_splits_into_scalars_and_blocks() {
        let file = parse(
            "[preset00]\r\n\
             fRating=2.000000\r\n\
             bTexWrap=1\r\n\
             zoom=1.000000\r\n\
             per_frame_2=b = 2;\r\n\
             per_frame_1=a = 1;\r\n\
             per_pixel_1=zoom = zoom + rad;\r\n",
        )
        .expect("parses");

        assert_eq!(file.key("frating"), Some("2.000000"));
        // Keys are case-insensitive: the corpus writes both spellings.
        assert_eq!(file.key("bTexWrap"), Some("1"));
        assert_eq!(file.number("zoom", 0.0), 1.0);
        assert_eq!(file.number("missing", 7.0), 7.0);
        // **Index order, not file order** — a preset whose lines are out of
        // sequence still means what it says — and joined directly, see
        // `join_code`.
        assert_eq!(file.block("per_frame"), "a = 1;b = 2;");
        assert_eq!(file.block("per_pixel"), "zoom = zoom + rad;");
        assert_eq!(file.block("per_vertex"), "");
    }

    /// A code line is split at the **first** `=`, because its value is full of
    /// them, and joined with a **newline**, because a real preset cuts a
    /// statement across two lines.
    #[test]
    fn a_code_line_keeps_everything_after_its_first_equals() {
        let file = parse("per_frame_1=x = y == 2;\nper_frame_2=z =\nper_frame_3= x + 1;\n")
            .expect("parses");
        assert_eq!(file.block("per_frame"), "x = y == 2;z = x + 1;");
    }

    /// **The join is direct, and a line comment ends where its line did.**
    ///
    /// Both halves matter and they pull against each other — see [`join_code`].
    /// A statement cut mid-identifier must rejoin, and a `//` comment must not
    /// swallow the rest of the program.
    #[test]
    fn a_statement_cut_mid_token_rejoins_and_a_comment_does_not_run_on() {
        // MilkDrop's writer chops at a fixed width: this is `is_beat`.
        let file = parse("per_frame_22=k1 =  is_\nper_frame_23=beat*2;\n").expect("parses");
        assert_eq!(file.block("per_frame"), "k1 =  is_beat*2;");

        let file = parse(
            "per_frame_1=//zoom = 1 + q1;\n\
             per_frame_2=zoom = 2;\n",
        )
        .expect("parses");
        assert_eq!(
            file.block("per_frame"),
            "zoom = 2;",
            "the comment must not reach the next line's code"
        );
    }

    /// A scalar key ending in a number is **not** a code line — the allowlist is
    /// what keeps it out of a program and in the roster where the converter can
    /// see it.
    #[test]
    fn a_scalar_key_ending_in_a_number_stays_a_scalar() {
        let file = parse("q1=3\nwavecode_0_enabled=1\nnMotionVectorsX=64\n").expect("parses");
        assert_eq!(file.key("q1"), Some("3"));
        assert_eq!(file.key("wavecode_0_enabled"), Some("1"));
        assert!(file.blocks.is_empty());
    }

    /// The per-wave and per-shape blocks are collected (Phase 5 counts them,
    /// Phase 4 consumes them) rather than silently folded into the main program.
    #[test]
    fn custom_wave_and_shape_blocks_are_collected_separately() {
        // **The appended-index form**, which is what MilkDrop actually writes for
        // these and which a `rsplit_once('_')` reading loses entirely.
        let file = parse(
            "wave_0_per_point1=x = 0.5;\n\
             wave_0_per_point2=y = 0.5;\n\
             wave_0_per_frame1=t1 = 1;\n\
             wave_0_init1=t2 = 2;\n\
             shape_2_per_frame1=x = 0.5;\n",
        )
        .expect("parses");
        assert_eq!(file.block("wave_0_per_point"), "x = 0.5;y = 0.5;");
        assert_eq!(file.block("wave_0_per_frame"), "t1 = 1;");
        assert_eq!(file.block("wave_0_init"), "t2 = 2;");
        assert_eq!(file.block("shape_2_per_frame"), "x = 0.5;");
        assert!(
            file.keys.is_empty(),
            "none of these may land in the scalar roster: {:?}",
            file.keys
        );
    }

    /// A shader block joins by newline with its backticks stripped — the exact
    /// opposite of the EEL join, because the writer never chops a shader line
    /// and a `#define` ends where its line does.
    #[test]
    fn a_shader_block_keeps_its_lines_and_loses_its_backticks() {
        let file = parse(
            "warp_1=`#define PI 3.14159\n\
             warp_2=`shader_body\n\
             warp_3=`{\n\
             warp_4=`    ret = tex2D(sampler_main, uv).xyz; // decay me\n\
             warp_5=`}\n",
        )
        .expect("parses");
        assert_eq!(
            file.block("warp"),
            "#define PI 3.14159\nshader_body\n{\n    ret = tex2D(sampler_main, uv).xyz; // decay me\n}\n"
        );
    }

    /// A file with nothing in it is a surfaced error rather than an empty preset
    /// that renders black.
    #[test]
    fn a_file_with_no_keys_is_not_a_preset() {
        assert_eq!(
            parse("[preset00]\n\n// nothing\n"),
            Err(MilkError::NotAPreset)
        );
    }

    /// The version line is what Phase 6's stop condition is priced against.
    #[test]
    fn the_version_line_says_whether_shaders_are_possible() {
        assert!(
            parse("MILKDROP_PRESET_VERSION=201\nzoom=1\n")
                .expect("parses")
                .is_milkdrop2()
        );
        assert!(!parse("zoom=1\n").expect("parses").is_milkdrop2());
    }
}
