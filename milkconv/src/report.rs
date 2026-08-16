//! **What actually converts** (Plan 0100 Phase 5): run the converter over a
//! directory of `.milk` files and report what happens to each.
//!
//! # This is a measurement and it asserts no threshold
//!
//! Per [ADR-0071], a coverage percentage is a property of *one corpus* and *one
//! converter* at *one moment*, so it is recorded with both named and is never a
//! gate. Nothing in this module fails, and nothing in the test suite reads its
//! numbers. What it produces is a **work list**: the ranked failure reasons are
//! how Phase 6's worth is decided.
//!
//! # The three counts, and why the third needs a GPU
//!
//! - **parse** — the `.milk` section layout read without error.
//! - **compile** — every EEL2 program in it turned into bytecode.
//! - **render non-blank** — the emitted preset loaded into the engine and put
//!   light on screen. That last one is the only one that catches a preset which
//!   converts perfectly and draws nothing, which is the failure class the plan's
//!   Risks call the worst for reputation, so it is worth the GPU. It is opt-in
//!   (`--render`) because it is also the only slow part.
//!
//! # Reading the ranking
//!
//! The plan states the distribution the census predicts **before** the converter
//! runs — a disk-texture class near 19 % and a shaderless-only success class near
//! 18 % — so a ranking that disagrees sharply with either is evidence about the
//! converter rather than about the corpus. [`Report::render`] prints those
//! predictions beside the measurement rather than leaving the comparison to
//! whoever reads it.
//!
//! [ADR-0071]: ../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::{convert, milk};

/// What became of one `.milk` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The section layout did not read.
    ParseFailed,
    /// It parsed, but a program did not compile.
    CompileFailed,
    /// It converted. Whether it *renders* is [`Report::blank`]'s business, since
    /// that needs a device.
    Converted,
}

/// One file's row.
#[derive(Debug, Clone)]
pub struct Row {
    /// The file, as given.
    pub path: PathBuf,
    /// How far it got.
    pub outcome: Outcome,
    /// Why not, when it did not.
    pub reason: Option<String>,
    /// The warning classes it raised, deduplicated — a preset with four textured
    /// shapes counts once for `shape-textured`, because the ranking is over
    /// *presets affected* and a per-occurrence count would let one file dominate.
    pub classes: Vec<&'static str>,
    /// Whether the source declares MilkDrop 2, and therefore carries HLSL that
    /// nothing translates yet.
    pub milkdrop2: bool,
    /// Set by the render pass: `Some(false)` means it put light on screen.
    pub blank: Option<bool>,
}

/// Everything one run measured.
#[derive(Debug, Default)]
pub struct Report {
    /// One row per file, in directory order.
    pub rows: Vec<Row>,
}

/// How a failure reason is collapsed for ranking.
///
/// A compile error carries the offending token, so the raw messages are almost
/// all distinct and a ranking over them is a list of one thousand ones. The
/// **shape** of the message is what a work list wants, so everything after the
/// first colon is dropped.
fn reason_class(reason: &str) -> &str {
    reason.split(':').next().unwrap_or(reason).trim()
}

/// Walk `dir` recursively for `.milk` files, sorted so a run is reproducible.
pub fn collect(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("milk"))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Convert one file and record what happened. Never fails: a file that does not
/// convert is a row, which is the whole point.
pub fn measure(path: &Path) -> Row {
    let mut row = Row {
        path: path.to_path_buf(),
        outcome: Outcome::ParseFailed,
        reason: None,
        classes: Vec::new(),
        milkdrop2: false,
        blank: None,
    };
    // Lossily, as the CLI does: `.milk` files are Windows-era text and a few
    // carry a Latin-1 byte in their title. Refusing one over its name would lose
    // a preset that converts fine.
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            row.reason = Some(format!("unreadable: {e}"));
            return row;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let file = match milk::parse(&text) {
        Ok(file) => file,
        Err(e) => {
            row.reason = Some(e.to_string());
            return row;
        }
    };
    row.milkdrop2 = file.is_milkdrop2();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preset");
    match convert::convert(&file, name) {
        Ok(converted) => {
            row.outcome = Outcome::Converted;
            for warning in &converted.warnings {
                if !row.classes.contains(&warning.class) {
                    row.classes.push(warning.class);
                }
            }
        }
        Err(e) => {
            row.outcome = Outcome::CompileFailed;
            row.reason = Some(e.to_string());
        }
    }
    row
}

/// Convert every file in `paths`, calling `progress` with `(done, total)` every
/// so often so a ten-thousand-file run is not a silent wait.
pub fn run(paths: &[PathBuf], mut progress: impl FnMut(usize, usize)) -> Report {
    let mut report = Report::default();
    for (index, path) in paths.iter().enumerate() {
        report.rows.push(measure(path));
        if index % 500 == 0 {
            progress(index, paths.len());
        }
    }
    progress(paths.len(), paths.len());
    report
}

/// How large the render probe captures. Small on purpose: the question is
/// "did anything light up", not "does it look right", and the whole corpus has
/// to fit in a coffee break.
const PROBE_SIZE: u32 = 64;
/// How many frames each probe advances. Enough that a feedback field has
/// something in it and a preset with a slow build-up is not called blank; short
/// enough that ten thousand of them finish.
const PROBE_FRAMES: u32 = 40;
/// Below this share of lit pixels a preset counts as blank. Not a gate — it is
/// the definition the count in the report is *of*, and it is stated so the number
/// can be read.
const BLANK_BELOW: f32 = 0.01;

/// Load each converted preset into a headless renderer and record whether it put
/// light on screen.
///
/// **The slow half, and the only one that needs a device.** Returns `Err` with a
/// message if no adapter is available, so a CI box without a GPU reports the two
/// static counts rather than failing (the same ADR-0016 policy the capture tests
/// take).
///
/// One renderer for the whole run, because building a device per preset is most
/// of the cost. `capture_preset` resets the scene between presets, so the shared
/// renderer does not make one preset's result depend on the last —
/// `a_bundle_drives_the_mesh_and_reruns_identically` is what holds that.
pub fn render_probe(
    report: &mut Report,
    mut progress: impl FnMut(usize, usize),
) -> Result<(), String> {
    use lmv_core::dsp::AnalysisFrame;
    use lmv_core::preset::Preset;
    use lmv_core::render::metrics::coverage;
    use lmv_core::render::{HeadlessOptions, Renderer};

    let mut renderer = Renderer::new_headless(HeadlessOptions {
        width: PROBE_SIZE,
        height: PROBE_SIZE,
        // The hardware adapter: this is a survey rather than a golden capture, so
        // what matters is speed and that the answer resembles what a user sees.
        prefer_software: false,
    })
    .map_err(|e| format!("no headless renderer: {e}"))?;

    // A fully-driven frame, so a preset gated on a band is not called blank for
    // want of a stimulus.
    let frame = AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; lmv_core::dsp::SPECTRUM_BINS],
        waveform: std::array::from_fn(|i| {
            (i as f32 / lmv_core::dsp::WAVE_SAMPLES as f32 * std::f32::consts::TAU * 4.0).sin()
        }),
        ..Default::default()
    };

    let total = report.rows.len();
    for (index, row) in report.rows.iter_mut().enumerate() {
        if index % 200 == 0 {
            progress(index, total);
        }
        if row.outcome != Outcome::Converted {
            continue;
        }
        // Re-convert rather than carrying every bundle's text through the whole
        // run: ten thousand preset bodies is hundreds of MB held for no reason,
        // and the conversion is the cheap half.
        let Ok(bytes) = std::fs::read(&row.path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let Ok(file) = milk::parse(&text) else {
            continue;
        };
        let name = row
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("preset");
        let Ok(converted) = convert::convert(&file, name) else {
            continue;
        };
        let Ok(mut preset) = Preset::from_toml_str(&converted.toml) else {
            // A bundle that converts and does not *load* is a converter bug, and
            // it is worth its own reason rather than being counted as blank.
            row.reason = Some(format!("emitted bundle does not load: {name}"));
            continue;
        };
        preset.name = "probe".into();
        renderer.set_presets(vec![preset]);
        match renderer.capture_preset("probe", &frame, PROBE_FRAMES) {
            Ok(image) => {
                let lit = coverage(&image, [0, 0, 0, 255], 10);
                row.blank = Some(lit < BLANK_BELOW);
            }
            Err(e) => row.reason = Some(format!("render failed: {e}")),
        }
    }
    progress(total, total);
    Ok(())
}

impl Report {
    /// How many rows reached each stage.
    fn counts(&self) -> (usize, usize, usize) {
        let parsed = self
            .rows
            .iter()
            .filter(|r| r.outcome != Outcome::ParseFailed)
            .count();
        let converted = self
            .rows
            .iter()
            .filter(|r| r.outcome == Outcome::Converted)
            .count();
        let drawn = self.rows.iter().filter(|r| r.blank == Some(false)).count();
        (parsed, converted, drawn)
    }

    /// A ranked `(label, count)` table, biggest first, with ties broken by label
    /// so a run is reproducible.
    fn ranked(counts: BTreeMap<&str, usize>) -> Vec<(&str, usize)> {
        let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        ranked
    }

    /// The whole report, as the text a human reads.
    ///
    /// `corpus` and `machine` are named in the output because ADR-0071 says a
    /// number like this is meaningless without them.
    pub fn render(&self, corpus: &str, machine: &str, rendered: bool) -> String {
        let total = self.rows.len();
        let (parsed, converted, drawn) = self.counts();
        let pct = |n: usize| -> String {
            if total == 0 {
                "  0.0 %".into()
            } else {
                format!("{:>5.1} %", 100.0 * n as f64 / total as f64)
            }
        };

        let mut out = String::new();
        let _ = writeln!(out, "milkconv --report");
        let _ = writeln!(out, "  corpus:  {corpus}");
        let _ = writeln!(out, "  machine: {machine}");
        let _ = writeln!(
            out,
            "\nThis is a MEASUREMENT and it asserts no threshold (ADR-0071): a coverage\n\
             percentage is a property of this corpus and this converter at this moment.\n\
             The ranked reasons below are the output that matters - they are the work list.\n"
        );

        let _ = writeln!(out, "  {:<28} {:>7}  share", "stage", "count");
        let _ = writeln!(out, "  {:-<28} {:->7}  {:-<7}", "", "", "");
        let _ = writeln!(out, "  {:<28} {total:>7}  {}", "files seen", pct(total));
        let _ = writeln!(out, "  {:<28} {parsed:>7}  {}", "parse", pct(parsed));
        let _ = writeln!(
            out,
            "  {:<28} {converted:>7}  {}",
            "compile",
            pct(converted)
        );
        if rendered {
            let _ = writeln!(
                out,
                "  {:<28} {drawn:>7}  {}",
                "render non-blank",
                pct(drawn)
            );
        } else {
            let _ = writeln!(
                out,
                "  {:<28} {:>7}    (pass --render)",
                "render non-blank", "-"
            );
        }

        // --- what did not convert, ranked ---
        let mut failures: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &self.rows {
            if let Some(reason) = row.reason.as_deref() {
                *failures.entry(reason_class(reason)).or_default() += 1;
            }
        }
        if failures.is_empty() {
            let _ = writeln!(out, "\nNothing failed to convert.");
        } else {
            let _ = writeln!(out, "\nWHY A FILE DID NOT CONVERT, ranked:");
            for (reason, count) in Self::ranked(failures) {
                let _ = writeln!(out, "  {count:>6}  {}  {reason}", pct(count));
            }
        }

        // --- what converted with a stated loss, ranked ---
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &self.rows {
            for class in &row.classes {
                *classes.entry(class).or_default() += 1;
            }
        }
        if !classes.is_empty() {
            let _ = writeln!(
                out,
                "\nWHAT A CONVERSION COULD NOT CARRY, ranked by presets affected:"
            );
            for (class, count) in Self::ranked(classes) {
                let _ = writeln!(out, "  {count:>6}  {}  {class}", pct(count));
            }
        }

        // --- the two predictions the census made, checked ---
        //
        // From Phase 6 on, a disk texture is a *conversion failure* (the shader
        // that samples it is rejected by name), so the class lives in the
        // failure reasons rather than the warning classes. Counted from both so
        // the prediction row survives the phase boundary it was written across.
        let disk = self
            .rows
            .iter()
            .filter(|r| {
                r.classes.contains(&"disk-texture")
                    || r.reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("disk-texture"))
            })
            .count();
        let shaderless = self.rows.iter().filter(|r| !r.milkdrop2).count();
        let _ = writeln!(
            out,
            "\nAGAINST THE CENSUS (taken 2026-08-16, before any of this was built).\n\
             A ranking that disagrees sharply with either row is evidence about the\n\
             CONVERTER rather than about the corpus - that is why the census came first.\n\
             The predictions are for the WHOLE 10 347-file corpus; point this at one\n\
             collection and they are the wrong prior, because the collections do not\n\
             have the same mix."
        );
        let _ = writeln!(out, "  {:<28} {:>8}   {:>8}", "", "predicted", "measured");
        let _ = writeln!(
            out,
            "  {:<28} {:>8}   {:>8}",
            "reads a disk texture",
            "19.0 %",
            pct(disk).trim()
        );
        let _ = writeln!(
            out,
            "  {:<28} {:>8}   {:>8}",
            "MilkDrop 1.x, no shaders",
            "18.0 %",
            pct(shaderless).trim()
        );

        // --- and what Phase 6 delivered ---
        let with_shaders = self
            .rows
            .iter()
            .filter(|r| r.milkdrop2 && r.outcome == Outcome::Converted)
            .count();
        let _ = writeln!(
            out,
            "\nWHAT PHASE 6 DELIVERS: {with_shaders} presets ({}) convert AND declare\n\
             MilkDrop 2 — their `warp` and `comp` HLSL is translated to WGSL and runs.\n\
             A MilkDrop 2 preset that no longer converts failed a shader-translation\n\
             rule (see the ranking above); before Phase 6 it converted WITHOUT its\n\
             shaders, which rendered something its author never drew.",
            pct(with_shaders).trim()
        );

        if rendered {
            let blank: Vec<&Row> = self.rows.iter().filter(|r| r.blank == Some(true)).collect();
            // Split by whether the source carries HLSL. Both halves are real
            // findings now: since Phase 6 an MD2 preset renders WITH its
            // translated shaders, so a blank one is a translation-fidelity item
            // rather than "the picture is in the missing shader".
            let shader_blank = blank.iter().filter(|r| r.milkdrop2).count();
            let plain_blank = blank.len() - shader_blank;
            let _ = writeln!(
                out,
                "\nCONVERTED BUT BLANK: {} presets ({}). These are the ones that cost\n\
                 reputation - they load without complaint and show nothing.\n\
                 (blank at under {:.0} % of pixels lit, after {PROBE_FRAMES} frames at \
                 {PROBE_SIZE}x{PROBE_SIZE})",
                blank.len(),
                pct(blank.len()).trim(),
                BLANK_BELOW * 100.0
            );
            let _ = writeln!(
                out,
                "  {shader_blank:>6}  declare MilkDrop 2 - their shaders RAN (Phase 6), so a\n\
                 \x20         blank one is a shader-fidelity finding\n\
                 \x20{plain_blank:>5}  are shaderless and blank - the Phases 1-5 work list"
            );
            for row in blank.iter().filter(|r| !r.milkdrop2).take(20) {
                let _ = writeln!(
                    out,
                    "    {}",
                    row.path
                        .file_name()
                        .unwrap_or(row.path.as_os_str())
                        .to_string_lossy()
                );
            }
            if plain_blank > 20 {
                let _ = writeln!(out, "    ... and {} more", plain_blank - 20);
            }
        }
        out
    }

    /// The same numbers as JSON, for a caller that wants to diff two runs.
    pub fn to_json(&self) -> String {
        let total = self.rows.len();
        let (parsed, converted, drawn) = self.counts();
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &self.rows {
            for class in &row.classes {
                *classes.entry(class).or_default() += 1;
            }
        }
        let mut failures: BTreeMap<&str, usize> = BTreeMap::new();
        for row in &self.rows {
            if let Some(reason) = row.reason.as_deref() {
                *failures.entry(reason_class(reason)).or_default() += 1;
            }
        }
        let mut out = String::from("{\n");
        let _ = writeln!(out, "  \"files\": {total},");
        let _ = writeln!(out, "  \"parse\": {parsed},");
        let _ = writeln!(out, "  \"compile\": {converted},");
        let _ = writeln!(out, "  \"render_non_blank\": {drawn},");
        let entries = |map: BTreeMap<&str, usize>| -> String {
            Self::ranked(map)
                .into_iter()
                .map(|(k, v)| format!("    \"{k}\": {v}"))
                .collect::<Vec<_>>()
                .join(",\n")
        };
        let _ = writeln!(out, "  \"failures\": {{\n{}\n  }},", entries(failures));
        let _ = writeln!(out, "  \"losses\": {{\n{}\n  }}", entries(classes));
        out.push_str("}\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compile reason is collapsed to its shape, or the ranking is a list of
    /// one thousand ones.
    #[test]
    fn a_reason_ranks_by_its_shape_and_not_by_its_token() {
        assert_eq!(
            reason_class("unknown function `foo` at line 3"),
            "unknown function `foo` at line 3"
        );
        assert_eq!(reason_class("per_frame: unexpected `)` at 12"), "per_frame");
        assert_eq!(
            reason_class("per_vertex: unexpected `}` at 40"),
            "per_vertex"
        );
    }

    /// The ranking is biggest-first and ties break by label, so two runs over the
    /// same corpus print the same table.
    #[test]
    fn the_ranking_is_reproducible() {
        let counts = BTreeMap::from([("b", 2usize), ("a", 2), ("c", 9)]);
        assert_eq!(Report::ranked(counts), vec![("c", 9), ("a", 2), ("b", 2)]);
    }

    /// An empty run divides by zero nowhere and still prints its header.
    #[test]
    fn an_empty_corpus_reports_rather_than_panics() {
        let text = Report::default().render("nothing", "test", false);
        assert!(text.contains("files seen"));
        assert!(text.contains("0.0 %"));
    }
}
