//! The `shot` CLI, run the way a user runs it (Plan 0032 Phase 2, ADR-0033 tier 4).
//!
//! `standalone/examples/shot.rs` is the headless-capture CLI the `preset-author`
//! lane self-verifies drafts through and the Plan 0013 visual-QA harness is built
//! on — and it had **no tests of any kind**, because `#[test]` does not run in an
//! `examples/` target. Moving its pure helpers into the `[lib]` (Plan 0031 Phase 1)
//! makes them assertable in-process but still leaves argument parsing, preset
//! resolution, exit codes, and file output unexercised. Those are only observable
//! from outside, so this suite spawns the built binary as a subprocess.
//!
//! `shot` is deliberately **not** promoted to a `[[bin]]` so `CARGO_BIN_EXE_shot`
//! would resolve it (ADR-0033 Alternative E): `image` is a dev-dependency precisely
//! to keep the PNG codec out of the shipped `lmv.exe`, and a `[[bin]]` does not get
//! dev-dependencies. It would also rename every documented invocation, including
//! ones in `.claude/skills/**` that cannot be edited. So the binary is located
//! under `target/<profile>/examples/` instead.
//!
//! `cargo nextest run` **does** build `examples/` targets — verified for this plan
//! by deleting `target/debug/examples/shot.exe` and watching `cargo nextest run -p
//! standalone` rebuild it — so neither CI nor the pre-push hook needs an explicit
//! build step. If the binary is missing anyway the tests fail with an actionable
//! message rather than passing silently.
//!
//! The GPU-free cases all exit before a renderer is constructed, so they run
//! everywhere. The rendering cases need a real adapter (`shot` asks for hardware,
//! not WARP) and **skip with a printed reason** where none exists — macOS has no
//! software Metal fallback (ADR-0016), and CI runners generally have no GPU. The
//! skip is keyed on the adapter error itself rather than on the OS, so an
//! adapterless Windows runner is handled too and any *other* failure still fails.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lmv_core::render::CaptureImage;
use lmv_core::render::metrics::frame_diff;

/// Substring of the error `shot` prints when no GPU adapter can be acquired
/// (`RenderError::RequestAdapter` -> "no suitable GPU adapter: ..."). Matching the
/// adapter failure specifically means an unrelated non-zero exit is never mistaken
/// for a skip.
const NO_ADAPTER: &str = "no suitable GPU adapter";

/// A shipped preset used for the library-resolution cases. If it is ever renamed,
/// these tests fail with a message saying so rather than something cryptic.
const SHIPPED_PRESET_FILE: &str = "presets/fragment_supernova.toml";
const SHIPPED_PRESET_NAME: &str = "Supernova";

/// A shipped preset whose `zoom` is gated on `tempo` — the evidence that
/// `--set tempo=` reaches the grammar rather than merely being accepted.
/// A minimal tempo-gated preset the tempo test writes to its own scratch dir.
/// It used to point at a *shipped* tempo-gated preset (`rose_zoom`), which
/// coupled a CLI-plumbing assertion to the content library — and broke the
/// day Plan 0075's cohort one retired the file. The probe owns its subject
/// now: two tempos, two camera depths, no dependency on what ships.
const TEMPO_GATED_SRC: &str = r#"
system = "fragment_field"
name = "probe_tempo_gate"
[params]
zoom = "select(tempo > 130, 1.9, 1.1)"
"#;

/// `core/tests/reactivity.rs`'s floor: below this, a difference is rasterization
/// noise rather than a preset responding.
const REACTIVITY_FLOOR: f32 = 0.02;

/// The repo root — `shot` is invoked from here so `presets` resolves and the
/// provenance label reads exactly `[--presets presets]`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("standalone/ has a parent")
        .to_path_buf()
}

/// Locate `target/<profile>/examples/shot[.exe]` by walking up from this test
/// binary. The test lives at `target/<profile>/deps/shot_cli-<hash>`, so the
/// `examples/` sibling is one or two levels up — searching the ancestors instead of
/// hardcoding the depth keeps this working under `CARGO_TARGET_DIR` and under a
/// `--target <triple>` layout, neither of which puts `target/debug` where you would
/// guess.
fn shot_bin() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary has a path");
    let name = format!("shot{}", std::env::consts::EXE_SUFFIX);
    for dir in exe.ancestors().skip(1).take(4) {
        let candidate = dir.join("examples").join(&name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "could not find the `shot` example next to {}; run \
         `cargo build -p standalone --example shot` first",
        exe.display()
    );
}

fn run(args: &[&str]) -> Output {
    Command::new(shot_bin())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("spawning the shot binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A unique scratch directory for output files, under `target/` so it never
/// escapes the build tree. Keyed by process id so concurrent test binaries do not
/// collide.
fn scratch(name: &str) -> PathBuf {
    let dir = repo_root()
        .join("target")
        .join("shot-cli-tests")
        .join(format!("{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// `true` when the run failed only because the machine has no usable GPU adapter,
/// in which case the caller prints a skip. Any other failure is a real one.
fn skipped_for_no_adapter(out: &Output) -> bool {
    if out.status.success() {
        return false;
    }
    if stderr(out).contains(NO_ADAPTER) {
        eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
        return true;
    }
    false
}

/// Assert a run failed, and that its stderr names the thing that was wrong.
fn assert_failed_naming(out: &Output, needle: &str, case: &str) {
    assert!(
        !out.status.success(),
        "{case}: expected a non-zero exit, got success\nstdout: {}",
        stdout(out)
    );
    let err = stderr(out);
    assert!(
        err.contains(needle),
        "{case}: stderr does not name `{needle}`\nstderr: {err}"
    );
}

// ---------------------------------------------------------------------------
// GPU-free cases — these exit before a renderer is built, so they run everywhere
// ---------------------------------------------------------------------------

/// Every bad-input path exits non-zero *and* says which input was bad. An agent
/// driving this CLI (the `preset-author` lane does) needs the failure to be
/// actionable, not just detectable.
#[test]
fn bad_input_exits_non_zero_and_names_the_offending_input() {
    assert_failed_naming(
        &run(&["--definitely-not-a-flag"]),
        "--definitely-not-a-flag",
        "unknown flag",
    );

    let missing_dir = "no_such_preset_dir";
    assert_failed_naming(
        &run(&["--presets", missing_dir, "--report"]),
        missing_dir,
        "missing --presets directory",
    );

    let missing_file = "no_such_preset_file.toml";
    assert_failed_naming(
        &run(&["--preset-file", missing_file, "--out", "unused.png"]),
        missing_file,
        "missing --preset-file",
    );

    // A single shot with no destination: the offending input is the *absent*
    // --out, so that is what the message must name.
    assert_failed_naming(
        &run(&["--preset", SHIPPED_PRESET_NAME]),
        "--out",
        "--preset without --out",
    );
}

/// The explicit library flags must **error** rather than quietly falling back to
/// some other preset library — the exit-code contract Plan 0015 pinned. A silent
/// fallback would hand back a capture of the wrong presets, which is the confusing
/// failure this CLI exists to avoid.
#[test]
fn explicit_library_flags_do_not_silently_fall_back() {
    let out = run(&["--presets", "no_such_preset_dir", "--report"]);
    assert!(!out.status.success(), "a bad --presets must not exit 0");
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        !combined.contains("embedded defaults"),
        "a bad --presets fell back to the embedded library instead of failing:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// Rendering cases — need a real adapter; skip with a reason where there is none
// ---------------------------------------------------------------------------

/// A single shot writes a real PNG at exactly the requested `--size`. This is the
/// whole point of the tool, and nothing checked that the file it wrote was a valid
/// image of the right dimensions.
#[test]
fn a_single_shot_writes_a_png_at_the_requested_size() {
    let out_dir = scratch("single");
    let png = out_dir.join("shot.png");
    let png_arg = png.to_string_lossy().into_owned();

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--size",
        "64x48",
        "--frames",
        "2",
        "--out",
        &png_arg,
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "single shot failed\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );
    assert!(png.is_file(), "no PNG at {}", png.display());

    let decoded = image::open(&png).expect("the written file decodes as an image");
    assert_eq!(
        (decoded.width(), decoded.height()),
        (64, 48),
        "the PNG is not the size --size asked for"
    );

    // A one-entry roster names itself, so --preset was not required (Plan 0015).
    assert!(
        stdout(&out).contains("--preset-file"),
        "the capture did not report its provenance:\n{}",
        stdout(&out)
    );
}

/// `--presets <dir>` wins the precedence chain and says so. The `[source]` label is
/// how a capture's provenance stops being a guess, and it is the direct evidence
/// that the app and the CLI share one resolver (ADR-0014).
#[test]
fn the_presets_flag_is_reported_as_the_source() {
    let out_dir = scratch("provenance");
    let png = out_dir.join("shot.png");
    let png_arg = png.to_string_lossy().into_owned();

    let out = run(&[
        "--presets",
        "presets",
        "--preset",
        SHIPPED_PRESET_NAME,
        "--size",
        "32x32",
        "--frames",
        "2",
        "--out",
        &png_arg,
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "capture from --presets failed (is `{SHIPPED_PRESET_NAME}` still a shipped \
         preset in {SHIPPED_PRESET_FILE}?)\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("[--presets presets]"),
        "stdout does not carry the --presets provenance label:\n{}",
        stdout(&out)
    );
}

/// `--set tempo=` must reach the expression grammar, not just be accepted by the
/// parser. The probe preset gates its `zoom` on `select(tempo > 130, 1.9, 1.1)`,
/// so a slow and a fast setting are two different camera depths — and the frames
/// have to differ by more than `core/tests/reactivity.rs`'s floor, or the flag is
/// decorative. This is the gap that kept `tempo` out of nearly every preset: an
/// author could not see it move.
#[test]
fn set_tempo_reaches_the_grammar_and_changes_the_render() {
    let dir = scratch("tempo");
    let slow = dir.join("slow.png");
    let fast = dir.join("fast.png");
    let probe = dir.join("tempo_gate.toml");
    std::fs::write(&probe, TEMPO_GATED_SRC).expect("write the tempo probe preset");

    // 60 frames at the fixed 1/60 s capture step is a full second; the probe
    // binds no `[smoothing]`, so both captures sit on their gated value.
    let shot_at = |tempo: &str, out: &Path| {
        run(&[
            "--preset-file",
            &probe.to_string_lossy(),
            "--set",
            &format!("tempo={tempo}"),
            "--size",
            "128x96",
            "--frames",
            "60",
            "--out",
            &out.to_string_lossy(),
        ])
    };

    let out = shot_at("90", &slow);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--set tempo=90 failed (does the probe preset still parse?)\n\
         stdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );
    let out = shot_at("160", &fast);
    assert!(
        out.status.success(),
        "--set tempo=160 failed: {}",
        stderr(&out)
    );

    let diff = frame_diff(&load_capture(&slow), &load_capture(&fast));
    eprintln!("tempo 90 vs 160: frame diff {diff:.4} (floor {REACTIVITY_FLOOR})");
    assert!(
        diff > REACTIVITY_FLOOR,
        "tempo=90 and tempo=160 rendered near-identically (frame diff {diff:.4} <= \
         {REACTIVITY_FLOOR}); --set tempo is not reaching the grammar"
    );
}

/// A filmstrip reports the levels it measured. The `preset-author` lane
/// calibrated a whole sweep against `--set bass=0.8` — far above anything real
/// material produces — because nothing ever showed it the real numbers.
#[test]
fn a_filmstrip_reports_the_band_levels_it_measured() {
    let dir = scratch("levels");
    let strip = dir.join("strip.png");

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--signal",
        "bass:60",
        "--strip",
        "2",
        "--size",
        "64x48",
        "--out",
        &strip.to_string_lossy(),
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "filmstrip failed\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );

    let text = stdout(&out);
    assert!(
        text.contains("audio levels"),
        "the filmstrip did not report its band levels:\n{text}"
    );
    for band in ["bass", "mid", "treb"] {
        assert!(
            text.lines().any(|l| l.trim_start().starts_with(band)),
            "no `{band}` row in the levels report:\n{text}"
        );
    }
    // The numbers have to be numbers — a row of `NaN` would be worse than no
    // report, since an author would calibrate against it.
    assert!(
        !text.contains("NaN") && !text.contains("inf"),
        "the levels report carries a non-finite value:\n{text}"
    );
}

/// Decode a PNG `shot` wrote back into the shape `lmv_core`'s metrics take, so a
/// CLI-level difference is measured with the same function the in-core harness
/// uses rather than a second, subtly different one.
fn load_capture(path: &Path) -> CaptureImage {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("decode {}: {e}", path.display()))
        .to_rgba8();
    CaptureImage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

/// A three-preset library across **two** families, written to a scratch directory
/// so `--report`'s cost is a property of this test rather than of how many presets
/// the project happens to ship.
///
/// This ran against `presets/` until 2026-08-04, where it was the slowest test in
/// CI by a factor of two — 948.9 s on `check (windows-latest)`, 61% of the whole
/// nextest wall clock, because `--report` probes every preset over the full window
/// and the shipped library had grown to 35. None of that expense reached the
/// assertion: the claim is that a hand-rolled emitter produces well-formed JSON,
/// which one family cannot show and thirty-five prove no better than three.
///
/// Three, not one, and two families rather than one, because the emitter writes
/// its separators by hand: two entries in `families` exercise the comma between
/// family objects and two presets inside `spectrum` the comma between preset
/// objects. A single-preset report — which the transient test already covers via
/// `--preset-file` — walks neither path.
///
/// Both families are line scenes, the cheapest thing `shot` can probe, and the two
/// spectrum entries differ in more than their name so the near-duplicate pass has
/// nothing to report.
fn tiny_report_library() -> PathBuf {
    let dir = scratch("json-report-library");
    for (file, toml) in [
        (
            "one.toml",
            "system = \"spectrum\"\nname = \"ReportOne\"\n\
             [params]\nbase = \"0.05\"\nscale = \"1.1\"\nthickness = \"6\"\n\
             hue = \"0.55\"\nbrightness = \"0.9\"\n",
        ),
        (
            "two.toml",
            "system = \"spectrum\"\nname = \"ReportTwo\"\n\
             [params]\nbase = \"0.4\"\nscale = \"0.6\"\nthickness = \"2\"\n\
             hue = \"0.12\"\nbrightness = \"0.7\"\n",
        ),
        (
            "three.toml",
            "system = \"parametric_curve\"\nname = \"ReportThree\"\n\
             [curve]\nfamily = \"maurer_rose\"\n\
             [params]\nn = \"6\"\nd = \"71\"\nsamples = \"361\"\nscale = \"0.8\"\n\
             spin = \"0\"\nhue = \"0.55\"\nthickness = \"2.0\"\nbrightness = \"0.9\"\n\
             draw_progress = \"1\"\n",
        ),
    ] {
        std::fs::write(dir.join(file), toml).expect("write report fixture");
    }
    dir
}

/// `--report --json` emits parseable JSON with the documented top-level shape. The
/// report is hand-rolled (no serde), so nothing but a consumer proves it is
/// well-formed — and the `preset-author` lane is that consumer.
///
/// Read over [`tiny_report_library`] rather than the shipped set: see there for why
/// the preset count was never part of the claim.
#[test]
fn the_json_report_is_well_formed_and_carries_its_top_level_keys() {
    let dir = tiny_report_library();
    let dir_arg = dir.to_string_lossy().into_owned();
    let out = run(&["--report", "--json", "--presets", &dir_arg]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--report --json failed\nstderr: {}",
        stderr(&out)
    );

    let json = stdout(&out);
    assert!(
        json_is_balanced(&json),
        "the report is not well-formed JSON (unbalanced braces/brackets/quotes):\n{json}"
    );
    for key in ["source", "families"] {
        assert!(
            top_level_keys(&json).iter().any(|k| k == key),
            "the report is missing top-level key `{key}`; keys were {:?}",
            top_level_keys(&json)
        );
    }

    // Both fixture families reached the map, so the balance check above was made
    // over a report that actually walked the hand-written separators — otherwise
    // a shrunken library would quietly turn this into a single-object test.
    for family in ["spectrum", "parametric_curve"] {
        assert!(
            json.contains(&format!("\"{family}\":")),
            "the report dropped the `{family}` family, so the emitter's \
             between-families comma went untested:\n{json}"
        );
    }
}

/// The transient columns carry a real measurement all the way out of the CLI
/// (Plan 0037 Phase 2), in both presentations.
///
/// The two `easing_*` fixtures are the right subject precisely because they are
/// twins apart from their `[smoothing]` table (see `core/tests/fixtures/`), so a
/// difference in the reported columns can only have come from the easing. This
/// asserts the *wiring* — that the numbers reach the table and the JSON and
/// differ with the table — not the measurement itself, which
/// `core/tests/easing.rs` owns at a probe window long enough to let the release
/// constant settle.
///
/// `--report`'s own window is shorter, and this comment used to say the fall
/// therefore "reads clamped rather than measured". It does not — nothing clamps;
/// `frames_to_settle` returns a plausible smaller number (Plan 0038 Phase 8).
/// That is why the cell carries a `+` here, which this test now also asserts
/// reaches both presentations. The separation is what matters either way.
#[test]
fn the_report_transient_columns_separate_the_two_easing_fixtures() {
    /// `(rise, fall, fall_marked)` off the one data row of a single-preset text
    /// report. A cell may carry a trailing `+` meaning *at least this many*
    /// (Plan 0038 Phase 8), which is stripped before parsing and reported
    /// separately.
    ///
    /// The columns are located **through the header, anchored from the row's
    /// end** — the easing fixtures are line-family presets, so their table
    /// also carries the trailing `geom` column (Plan 0075 Phase 2), while a
    /// family without a line seam prints without it. Counting from the end of
    /// the header keeps the parse right in both shapes and stays indifferent
    /// to how the preset name itself tokenizes.
    fn columns(report: &str) -> (u32, u32, bool) {
        let header: Vec<&str> = report
            .lines()
            .find(|l| l.trim_start().starts_with("preset"))
            .unwrap_or_else(|| panic!("no table header in the report:\n{report}"))
            .split_whitespace()
            .collect();
        let from_end = |name: &str| {
            let idx = header
                .iter()
                .position(|c| *c == name)
                .unwrap_or_else(|| panic!("no `{name}` column in the header: {header:?}"));
            header.len() - idx
        };
        let (rise_back, fall_back) = (from_end("rise"), from_end("fall"));
        let row = report
            .lines()
            .find(|l| l.trim_start().starts_with("fixture_easing"))
            .unwrap_or_else(|| panic!("no fixture row in the report:\n{report}"));
        let cols: Vec<&str> = row.split_whitespace().collect();
        let n = cols.len();
        let parse = |s: &str| {
            s.trim_end_matches('+')
                .parse()
                .unwrap_or_else(|_| panic!("`{s}` in {row}"))
        };
        let fall_cell = cols[n - fall_back];
        (
            parse(cols[n - rise_back]),
            parse(fall_cell),
            fall_cell.ends_with('+'),
        )
    }

    let probe = |file: &str| -> Option<(u32, u32, bool)> {
        let out = run(&["--report", "--preset-file", file]);
        if skipped_for_no_adapter(&out) {
            return None;
        }
        assert!(
            out.status.success(),
            "--report on {file} failed\nstderr: {}",
            stderr(&out)
        );
        let text = stdout(&out);
        assert!(
            text.contains("rise") && text.contains("fall"),
            "the report header lost its transient columns:\n{text}"
        );
        Some(columns(&text))
    };

    let Some((scalar_rise, scalar_fall, _)) = probe("core/tests/fixtures/easing_scalar.toml")
    else {
        return;
    };
    let Some((asym_rise, asym_fall, asym_fall_marked)) =
        probe("core/tests/fixtures/easing_asymmetric.toml")
    else {
        return;
    };
    println!("scalar {scalar_rise}/{scalar_fall}, asymmetric {asym_rise}/{asym_fall}");

    // The marker reaches the text table, and this fixture is the case that
    // earns it: `release = 0.5` against a 0.8 s window leaves ~4x more travel
    // undone than `PROBE_SETTLE_TOL` allows, so the fall here is a lower bound
    // and the report must say so rather than printing it bare.
    assert!(
        asym_fall_marked,
        "the asymmetric fall ({asym_fall}) reached the table unmarked — a 0.5 s \
         release cannot settle in --report's 0.8 s window, so `segment_settled` \
         is not wired into the cell"
    );

    assert!(
        scalar_rise > 0 && scalar_fall > 0,
        "the scalar fixture reported no transient at all ({scalar_rise}/{scalar_fall}) — \
         the column is not measuring anything"
    );
    assert!(
        asym_rise * 3 < scalar_rise,
        "the pair's fast attack did not reach the report: {asym_rise} frames \
         against the scalar's {scalar_rise}"
    );
    assert!(
        asym_fall > asym_rise * 3,
        "the report did not separate the two tables: asymmetric read \
         {asym_rise}/{asym_fall}"
    );

    // ...and the same numbers are in the JSON, as integers under `transient`.
    let out = run(&[
        "--report",
        "--json",
        "--preset-file",
        "core/tests/fixtures/easing_asymmetric.toml",
    ]);
    assert!(out.status.success(), "json report: {}", stderr(&out));
    let json = stdout(&out);
    assert!(
        json_is_balanced(&json),
        "the report with the transient object is not well-formed JSON:\n{json}"
    );
    for key in [
        "\"transient\":",
        "\"rise_frames\":",
        "\"fall_frames\":",
        "\"ratio\":",
        // The text table's `+` has a JSON equivalent, or a consumer reading only
        // the counts cannot tell a truncated response from a settled one.
        "\"fall_settled\":false",
    ] {
        assert!(
            json.contains(key),
            "the JSON report is missing {key}:\n{json}"
        );
    }
    assert!(
        json.contains(&format!("\"rise_frames\":{asym_rise}")),
        "the JSON rise ({asym_rise} in the text table) is missing or differs:\n{json}"
    );
}

// ---------------------------------------------------------------------------
// Minimal JSON structural checks
//
// `serde_json` is not a dependency of this crate and adding one for a test would
// spend NFR section 4's dependency budget on a hundred lines of scanning. These
// two functions are string-aware and escape-aware, which is all the report's fixed
// numeric schema needs.
// ---------------------------------------------------------------------------

/// Braces, brackets and quotes all close, and no structural character inside a
/// string is counted.
fn json_is_balanced(s: &str) -> bool {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in s.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            // Written as an inner `match` rather than a guard on the arm: the
            // guard form clippy suggests would make `stack.pop()` a side effect
            // inside a condition, which reads as though nothing was consumed when
            // the closer matches.
            '}' | ']' => match stack.pop() {
                Some(expected) if expected == c => {}
                _ => return false,
            },
            _ => {}
        }
    }
    !in_string && stack.is_empty()
}

/// The keys of the outermost object, in order of appearance: quoted strings that
/// sit at nesting depth 1 and are immediately followed by a colon.
fn top_level_keys(s: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            '"' => {
                let mut key = String::new();
                let mut escaped = false;
                for c in chars.by_ref() {
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '"' => break,
                        _ => key.push(c),
                    }
                }
                // A key is a string at the outermost object depth followed by ':'.
                if depth == 1 && chars.peek() == Some(&':') {
                    keys.push(key);
                }
            }
            _ => {}
        }
    }
    keys
}

#[test]
fn the_json_helpers_reject_what_they_should() {
    assert!(json_is_balanced(r#"{"a":[1,2],"b":{"c":3}}"#));
    assert!(
        json_is_balanced(r#"{"a":"}]{["}"#),
        "structure inside a string"
    );
    assert!(json_is_balanced(r#"{"a":"esc\"aped"}"#));
    assert!(!json_is_balanced(r#"{"a":[1,2}"#), "mismatched closer");
    assert!(!json_is_balanced(r#"{"a":1"#), "unclosed object");
    assert!(
        !json_is_balanced(r#"{"a":"unterminated}"#),
        "unclosed string"
    );

    assert_eq!(top_level_keys(r#"{"a":1,"b":{"c":2}}"#), vec!["a", "b"]);
    assert_eq!(
        top_level_keys(r#"{"source":"x","families":{"swarm":{"presets":{}}}}"#),
        vec!["source", "families"],
        "nested keys must not be reported as top level"
    );
}
