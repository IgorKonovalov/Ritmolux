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
//! to keep the PNG codec out of the shipped `ritmolux.exe`, and a `[[bin]]` does not get
//! dev-dependencies. It would also rename every documented invocation, including
//! ones in `.claude/skills/**` that cannot be edited. So the binary is located
//! under `target/<profile>/examples/` instead.
//!
//! `cargo nextest run` **does** build `examples/` targets — verified by deleting
//! `target/debug/examples/shot.exe` and watching `cargo nextest run -p
//! standalone` rebuild it — so neither CI nor the pre-push hook needs an
//! explicit build step. If the binary is missing anyway the tests fail with an
//! actionable message rather than passing silently.
//!
//! The GPU-free cases all exit before a renderer is constructed, so they run
//! everywhere. The rendering cases need a real adapter (`shot` asks for hardware,
//! not WARP) and **skip with a printed reason** where none exists — macOS has no
//! software Metal fallback (ADR-0016), and CI runners generally have no GPU. The
//! skip is keyed on the adapter error itself rather than on the OS, so an
//! adapterless Windows runner is handled too and any *other* failure still fails.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rlx_core::render::CaptureImage;
use rlx_core::render::metrics::frame_diff;

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
/// `--set tempo=` reaches the grammar rather than merely being accepted. A
/// minimal tempo-gated preset the tempo test writes to its own scratch dir.
/// Pointing at a *shipped* tempo-gated preset (`rose_zoom`) coupled a
/// CLI-plumbing assertion to the content library, and broke the day Plan
/// 0075's cohort one retired the file. The probe owns its subject: two
/// tempos, two camera depths, no dependency on what ships.
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

/// A unique scratch directory for output files, inside the build tree so it never
/// escapes it. Keyed by process id so concurrent test binaries do not collide.
///
/// The root is `CARGO_TARGET_TMPDIR`, which cargo sets to a per-test-binary
/// directory inside the target directory it is actually writing to, and which
/// exists for exactly this. Deriving it from `CARGO_MANIFEST_DIR` instead names
/// the SOURCE location, so the path tracks the worktree rather than the build
/// output and the sentence above is a promise it cannot keep under any
/// `build.target-dir`. Backlog 0160 and ADR-0147 carry the reasoning.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
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

/// `--frame-at` exists because neither existing path produces a documentation
/// image (Plan 0088 Phase 1, ADR-0100): `--frames` reaches full size under
/// **silence**, and `--at` runs the real analyzer but tiles its output to a fixed
/// height with a gutter round it. So the assertion that matters is a size
/// comparison between the two paths on the *same* hop — not merely that a file
/// appeared.
#[test]
fn frame_at_writes_the_full_size_where_the_same_hop_as_a_strip_does_not() {
    let dir = scratch("frame-at");
    let frame = dir.join("frame.png");
    let tile = dir.join("tile.png");
    const HOP: &str = "30"; // past FILMSTRIP_WARMUP, inside the 4 s clip
    const SIZE: (u32, u32) = (160, 120);

    let common = [
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--signal",
        "dynamic:110",
        "--size",
        "160x120",
    ];

    let out = run(&[
        &common[..],
        &["--frame-at", HOP, "--out", &frame.to_string_lossy()],
    ]
    .concat());
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--frame-at failed\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );

    let decoded = image::open(&frame).expect("the written file decodes as an image");
    assert_eq!(
        (decoded.width(), decoded.height()),
        SIZE,
        "--frame-at must write the frame at --size, unscaled and unbordered"
    );

    // The stdout line names the hop it captured, so a manifest entry and its
    // image can be matched up from the run log alone.
    let text = stdout(&out);
    assert!(
        text.contains(&format!("hop {HOP}")),
        "the capture did not name the hop it took:\n{text}"
    );
    // ...and the level table is the filmstrip's, unchanged — same clip, same
    // analysis, only the write differs.
    assert!(
        text.contains("audio levels"),
        "--frame-at dropped the band-level report the strip prints:\n{text}"
    );

    // The counter-case that makes the size assertion mean something: `--at` on
    // the very same hop comes back scaled to STRIP_H with a gutter, which is
    // neither dimension above.
    let out = run(&[
        &common[..],
        &["--at", HOP, "--out", &tile.to_string_lossy()],
    ]
    .concat());
    assert!(out.status.success(), "--at failed\n{}", stderr(&out));
    let tiled = image::open(&tile).expect("the strip decodes");
    assert_ne!(
        (tiled.width(), tiled.height()),
        SIZE,
        "the strip path was expected to tile, not to write the frame at --size"
    );
}

/// The two ways of asking for `--frame-at` that cannot be honoured. Both are
/// GPU-free — they fail in the parser, before a renderer is built.
#[test]
fn frame_at_rejects_a_second_hop_source_and_a_missing_clip() {
    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--signal",
            "dynamic:110",
            "--frame-at",
            "30",
            "--at",
            "30",
            "--out",
            "unused.png",
        ]),
        "--frame-at and --at",
        "both flags choose a hop",
    );

    // Without a clip there is nothing to advance through, and the message has to
    // name the flags that would supply one.
    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--frame-at",
        "30",
        "--out",
        "unused.png",
    ]);
    assert_failed_naming(&out, "--frame-at needs audio", "no --signal or --audio");
    assert!(
        stderr(&out).contains("--signal") && stderr(&out).contains("--audio"),
        "the message must name what is missing:\n{}",
        stderr(&out)
    );

    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--signal",
            "dynamic:110",
            "--frame-at",
            "9999",
            "--out",
            "unused.png",
        ]),
        "analysis hops long",
        "a hop past the end of the clip",
    );
}

/// Decode a PNG `shot` wrote back into the shape `rlx_core`'s metrics take, so a
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

/// A preset whose **only** audio binding is `bloom_amount` — backlog 0088's
/// subject, written by the test so the claim cannot drift with the shipped
/// library. The figure is static and small (coverage ~0.03 at the report
/// size), so the bloom halo's change is concentrated: exactly the class the
/// whole-frame mean dilutes.
const BLOOM_ONLY_SRC: &str = r#"
system = "star_pattern"
name = "probe_bloom_only"
[generator]
tiling            = "12"
contact_angle_deg = 20
[params]
variant       = "1"
rotation      = "0.4 * time"
hue           = "0.55"
draw_progress = "1"
thickness     = "1.8"
scale         = "0.6"
brightness    = "0.85"
bloom_amount  = "clamp(bass * 1.2, 0, 0.9)"
"#;

/// The number following `"<band>":` inside the named object of `preset`'s JSON
/// entry — e.g. `value_for(&json, "reactivity_footprint", "bass")`. String
/// surgery rather than a JSON crate, deliberately: the report's JSON is
/// hand-rolled and these tests are its only consumer-side proof.
fn value_for(json: &str, object: &str, band: &str) -> f32 {
    let obj_start = json
        .find(&format!("\"{object}\":{{"))
        .unwrap_or_else(|| panic!("`{object}` object missing from report JSON:\n{json}"));
    let tail = &json[obj_start..];
    let key = format!("\"{band}\":");
    let val_start = tail
        .find(&key)
        .unwrap_or_else(|| panic!("`{band}` missing from `{object}`:\n{tail}"))
        + key.len();
    let rest = &tail[val_start..];
    let end = rest
        .find([',', '}'])
        .unwrap_or_else(|| panic!("unterminated number in `{object}.{band}`"));
    rest[..end]
        .trim()
        .parse::<f32>()
        .unwrap_or_else(|e| panic!("`{object}.{band}` is not a number ({e}): {}", &rest[..end]))
}

/// **The report can tell a bloom world from a dead one** — Plan 0077 Phase 4's
/// done-when, run the way the content lane runs it. A preset spending all its
/// reactivity on `bloom_amount` reads ~0.000 in the mean band columns (that
/// reading is deliberately unchanged — it is the historical statistic), and
/// **visibly nonzero** in the footprint reading, which divides the same
/// differential by the union of lit pixels instead of the whole frame
/// (`metrics::footprint_diff`, ADR-0091). The `flash`-lever workaround existed
/// only because no column could see this; this is the assertion that it is no
/// longer necessary.
#[test]
fn the_footprint_reading_sees_reactivity_spent_on_bloom() {
    let dir = scratch("bloom-only-report");
    let file = dir.join("bloom_only.toml");
    std::fs::write(&file, BLOOM_ONLY_SRC).expect("write the bloom-only fixture");
    let file_arg = file.to_string_lossy().into_owned();

    let out = run(&["--report", "--json", "--preset-file", &file_arg]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--report --json on the bloom fixture failed\nstderr: {}",
        stderr(&out)
    );
    let json = stdout(&out);

    let mean_bass = value_for(&json, "reactivity", "bass");
    let footprint_bass = value_for(&json, "reactivity_footprint", "bass");
    eprintln!("bloom-only fixture: mean bass {mean_bass}, footprint bass {footprint_bass}");

    // The historical column still under-reads the bloom world — sub-floor,
    // which is the backlog's "~0.000". If this ever rises past the reactivity
    // floor the mean statistic has changed, and the footprint block's reason
    // to exist should be re-examined.
    assert!(
        mean_bass < REACTIVITY_FLOOR,
        "the mean bass column reads {mean_bass} — no longer sub-floor, so the \
         whole-frame statistic has changed underneath this test"
    );
    // The footprint reading sees it, at or past the same floor the reactivity
    // suite counts as a real response.
    assert!(
        footprint_bass >= REACTIVITY_FLOOR,
        "the footprint reading must see bloom-spent reactivity: bass reads \
         {footprint_bass} against the {REACTIVITY_FLOOR} floor"
    );
    // And the structural bands the preset never binds stay dead in both
    // readings — the instrument gained sensitivity, not noise.
    for band in ["mid", "treb"] {
        let v = value_for(&json, "reactivity_footprint", band);
        assert!(
            v < REACTIVITY_FLOOR,
            "`{band}` is unbound in the fixture but the footprint reading \
             claims {v} — the new block is inventing reactivity"
        );
    }
}

// ---------------------------------------------------------------------------
// The long-run horizon (Plan 0085 Phase 1, ADR-0099)
// ---------------------------------------------------------------------------

/// A world with a **known accumulation axis**: a de Jong attractor whose trail
/// `fade` is 0.999, so deposited ink persists for ~1000 frames and the field
/// fills monotonically. Nothing here is audio-driven and no coefficient moves,
/// which is the point — the drift is the accumulation and nothing else.
///
/// Written by the test rather than pointed at a shipped preset, for the reason
/// the tempo probe learned: coupling an instrument's proof to the content
/// library breaks it the day a world is retuned.
const HORIZON_SUBJECT_SRC: &str = r#"
system = "attractor"
name = "horizon_subject"
[particles]
family = "de_jong"
[params]
a = "1.641"
b = "1.902"
c = "0.316"
d = "1.525"
size = "0.6"
fade = "0.999"
saturation = "0.3"
hue_center = "0.5"
"#;

/// The **static control**, and the half that makes the subject's numbers mean
/// something. A star pattern with a constant rotation and no time, audio or
/// feedback term: it renders the same frame forever, so every statistic must
/// come back flat. Without it, a drifting series would only show that the
/// instrument emits varying numbers.
const HORIZON_CONTROL_SRC: &str = r#"
system = "star_pattern"
name = "horizon_control"
[generator]
tiling            = "12"
contact_angle_deg = 20
[params]
variant       = "1"
rotation      = "0.4"
hue           = "0.55"
draw_progress = "1"
thickness     = "1.8"
scale         = "0.6"
brightness    = "0.85"
"#;

/// Horizon runs are sized so the whole case is a couple of seconds: three
/// simulated seconds at 48x48 is 181 renders. The mode is minutes of wall clock
/// at a useful horizon — that cost belongs to the lane running it, never to CI.
const HORIZON_SIZE: &str = "48x48";
const HORIZON_INTERVAL: &str = "1";

/// Write `src` to a scratch `.toml` and return the path as an owned argument.
fn fixture(dir_name: &str, file: &str, src: &str) -> String {
    let path = scratch(dir_name).join(file);
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    path.to_string_lossy().into_owned()
}

/// The `samples` array of a horizon report, verbatim.
fn samples_array(json: &str) -> &str {
    let start = json
        .find("\"samples\":[")
        .unwrap_or_else(|| panic!("no samples array in:\n{json}"));
    let tail = &json[start..];
    let end = tail
        .find(']')
        .unwrap_or_else(|| panic!("unterminated samples array in:\n{json}"));
    &tail[..=end]
}

/// **Both determinism properties, on rendered pixels rather than on
/// arithmetic** (Plan 0085 Phase 1's done-when).
///
/// A drift verdict recorded in a world's header is worth nothing unless the same
/// world at the same horizon produces the same rows — that is the first claim.
/// The second is subtler and is what lets a ten-minute run be read against a
/// two-minute one: the statistics at interval *k* must not depend on how far the
/// run was asked to go. Both are asserted on the JSON text, so a difference in
/// any digit of any row fails.
#[test]
fn a_horizon_is_reproducible_and_does_not_depend_on_its_own_length() {
    let subject = fixture("horizon", "subject.toml", HORIZON_SUBJECT_SRC);
    let horizon = |minutes: &str| {
        run(&[
            "--preset-file",
            &subject,
            "--horizon",
            minutes,
            "--interval",
            HORIZON_INTERVAL,
            "--size",
            HORIZON_SIZE,
            "--json",
        ])
    };

    let first = horizon("0.05");
    if skipped_for_no_adapter(&first) {
        return;
    }
    assert!(
        first.status.success(),
        "the horizon run failed\nstdout: {}\nstderr: {}",
        stdout(&first),
        stderr(&first)
    );
    let first = stdout(&first);
    assert!(
        json_is_balanced(&first),
        "the horizon report is not well-formed JSON:\n{first}"
    );

    // Same request, second run: every row identical.
    let again = horizon("0.05");
    assert!(again.status.success(), "the repeat run failed");
    assert_eq!(
        samples_array(&first),
        samples_array(&stdout(&again)),
        "two runs of the same world at the same horizon produced different \
         rows — a recorded drift verdict would not be reproducible"
    );

    // Twice the horizon: the rows it shares with the short run are the same
    // rows. Asserted as a prefix, because the long run has more of them.
    let long = horizon("0.1");
    assert!(long.status.success(), "the long run failed");
    let long = stdout(&long);
    let (short_rows, long_rows) = (samples_array(&first), samples_array(&long));
    assert!(
        long_rows.len() > short_rows.len(),
        "the longer horizon did not produce more rows: {long_rows}"
    );
    // Drop the short array's closing bracket and require the rest to be a
    // prefix of the long one.
    let shared = short_rows.trim_end_matches(']');
    assert!(
        long_rows.starts_with(shared),
        "an interval's statistics changed when a longer horizon was requested:\n\
         short: {short_rows}\nlong:  {long_rows}"
    );
}

/// **The non-vacuity half**: an accumulating world reads as a one-way trend
/// while a static control reads flat.
///
/// This is what separates an instrument from a number generator. The subject's
/// trail `fade` gives it a known accumulation axis and it must show up as a
/// monotone series; the control renders one frame over and over and every
/// statistic must come back with zero travel. Either half alone proves nothing.
#[test]
fn a_horizon_separates_an_accumulating_world_from_a_static_control() {
    let subject = fixture("horizon", "subject.toml", HORIZON_SUBJECT_SRC);
    let control = fixture("horizon", "control.toml", HORIZON_CONTROL_SRC);
    let horizon = |file: &str| {
        run(&[
            "--preset-file",
            file,
            "--horizon",
            "0.05",
            "--interval",
            HORIZON_INTERVAL,
            "--size",
            HORIZON_SIZE,
            "--json",
        ])
    };

    let out = horizon(&subject);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "the subject run failed: {}",
        stderr(&out)
    );
    let subject = stdout(&out);

    let out = horizon(&control);
    assert!(
        out.status.success(),
        "the control run failed: {}",
        stderr(&out)
    );
    let control = stdout(&out);

    // The subject drifts one way. `coverage` is the statistic the accumulation
    // moves most directly — the field fills — so it carries the claim, and the
    // floor is `core/tests/reactivity.rs`'s: below it, a difference is noise.
    let drift = value_for(&subject, "coverage", "delta");
    let direction = value_for(&subject, "coverage", "monotone");
    eprintln!("subject coverage: delta {drift}, monotone {direction}");
    assert!(
        drift.abs() > REACTIVITY_FLOOR,
        "the accumulating world's coverage travelled {drift}, under the \
         {REACTIVITY_FLOOR} noise floor — the horizon cannot see its own subject"
    );
    assert_eq!(
        direction, 1.0,
        "the accumulation is one-way, so every step should have agreed with it"
    );

    // The control does not move at all. Exact zeros, not a tolerance: the same
    // frame rendered twice is the same bytes, so any travel here would be a
    // defect in the instrument rather than a property of the world.
    for stat in ["coverage", "peak/mean", "footprint"] {
        let delta = value_for(&control, stat, "delta");
        let monotone = value_for(&control, stat, "monotone");
        assert_eq!(
            (delta, monotone),
            (0.0, 0.0),
            "the static control drifted on `{stat}`: delta {delta}, monotone \
             {monotone} — the instrument is reporting motion that is not there"
        );
    }
}

/// A horizon cannot be driven by a clip, and says so rather than silently
/// ignoring one of the two stimuli it was handed. GPU-free — it fails in the
/// parser.
#[test]
fn a_horizon_rejects_a_clip_as_its_stimulus() {
    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--horizon",
            "0.05",
            "--signal",
            "dynamic:110",
        ]),
        "--horizon holds a single stimulus",
        "a horizon with --signal",
    );
    assert_failed_naming(
        &run(&["--preset-file", SHIPPED_PRESET_FILE, "--horizon", "soon"]),
        "--horizon expects minutes",
        "a non-numeric horizon",
    );
    // An interval longer than the whole run samples nothing; the error names
    // the flag that has to move.
    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--horizon",
            "0.05",
            "--interval",
            "600",
        ]),
        "longer than",
        "an interval past the horizon",
    );
}

// ---------------------------------------------------------------------------
// The offline render mode (Plan 0101 Phase 1, ADR-0114)
// ---------------------------------------------------------------------------

/// Wrap interleaved 16-bit samples in a canonical 44-byte-header WAV.
///
/// `--render` takes a **file**, and this repo commits no audio (see
/// `assets/test/`), so every render fixture is written by the test that needs
/// it. Same header layout `shot::wav`'s own tests build, for the same reason:
/// the real parser has to accept it.
fn wav_bytes(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let block_align = channels * 2;
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // uncompressed PCM
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    b.extend_from_slice(&block_align.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

/// A `secs`-long stereo clip with real dynamics, written to the scratch tree.
///
/// `dynamic_groove` rather than a steady tone: a render whose analysis never
/// moves would pass a determinism check while proving nothing about the two
/// clocks, since every frame would carry the same stimulus either way.
fn render_clip(name: &str, sample_rate: u32, secs: f32) -> PathBuf {
    let format = rlx_core::audio::AudioFormat {
        sample_rate,
        channels: 2,
    };
    let pcm = rlx_core::signal::dynamic_groove(110.0, secs, format);
    let samples: Vec<i16> = pcm
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16)
        .collect();
    let path = scratch("render").join(name);
    std::fs::write(&path, wav_bytes(2, sample_rate, &samples))
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    path
}

/// Frames in a Y4M stream: one `FRAME\n` marker each.
///
/// Counted by scanning for the marker rather than by dividing the byte length,
/// because a truncated final plane would divide out to the right answer and is
/// exactly the failure worth catching.
fn y4m_frame_count(stream: &[u8]) -> usize {
    stream.windows(6).filter(|w| *w == b"FRAME\n").count()
}

/// **Phase 1's done-when, both halves.** Two runs of the same command produce
/// **byte-identical** streams, and the stream holds `ceil(clip_seconds x fps)`
/// frames.
///
/// Determinism is the property this whole feature rests on — it is what
/// separates an offline render from a screen capture, and the reason the feature
/// is cheap here and impossible for every live visualizer — so it is asserted
/// rather than inferred from the fact that `dt` is injected.
#[test]
fn a_render_is_byte_identical_across_runs_and_has_the_frame_count_its_length_implies() {
    const SECS: f32 = 2.0;
    const FPS: u32 = 30;
    let clip = render_clip("clip.wav", 48_000, SECS);
    let clip_arg = clip.to_string_lossy().into_owned();

    let render = || {
        run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--render",
            &clip_arg,
            "--fps",
            "30",
            "--size",
            "32x24",
        ])
    };

    let first = render();
    if skipped_for_no_adapter(&first) {
        return;
    }
    assert!(
        first.status.success(),
        "--render failed\nstderr: {}",
        stderr(&first)
    );

    // Self-describing: the geometry, rate and colour range are on the wire, so a
    // consumer needs nothing from the command line (ADR-0114).
    let header = b"YUV4MPEG2 W32 H24 F30:1 Ip A1:1 C444 XCOLORRANGE=FULL\n";
    assert!(
        first.stdout.starts_with(header),
        "the stream does not open with the declared header; first 64 bytes: {:?}",
        &first.stdout[..first.stdout.len().min(64)]
    );

    // ceil(2.0 s x 30 fps) = 60.
    let expected = (SECS * FPS as f32).ceil() as usize;
    assert_eq!(
        y4m_frame_count(&first.stdout),
        expected,
        "the stream does not hold ceil(clip_seconds x fps) frames"
    );
    // ...and the payload is the exact size that frame count implies at C444,
    // so a short final plane cannot hide behind a right-looking marker count.
    assert_eq!(
        first.stdout.len(),
        header.len() + expected * (b"FRAME\n".len() + 32 * 24 * 3),
        "the stream is not header + N complete 4:4:4 frames"
    );

    // The human-readable half went to stderr, because stdout is the video.
    assert!(
        stderr(&first).contains("frames written"),
        "the render printed no summary on stderr:\n{}",
        stderr(&first)
    );

    let again = render();
    assert!(again.status.success(), "the repeat run failed");
    assert_eq!(
        first.stdout.len(),
        again.stdout.len(),
        "two runs produced different stream lengths"
    );
    assert!(
        first.stdout == again.stdout,
        "two runs of the same render produced different bytes — the mode is not \
         deterministic, which is the one property that distinguishes it from a \
         screen capture"
    );
}

/// The render mode's rejections, all GPU-free — they fail in the parser.
#[test]
fn a_render_rejects_a_second_clip_a_decimal_rate_and_a_pointless_out() {
    let clip = render_clip("reject.wav", 48_000, 0.5);
    let clip_arg = clip.to_string_lossy().into_owned();

    // Two clips would mean silently ignoring one of them.
    assert_failed_naming(
        &run(&["--render", &clip_arg, "--signal", "dynamic:110"]),
        "--signal",
        "--render with a second clip",
    );
    assert_failed_naming(
        &run(&["--render", &clip_arg, "--horizon", "0.05"]),
        "--horizon",
        "--render with a horizon",
    );

    // stdout is the frame stream, so `--out` has nothing to name.
    assert_failed_naming(
        &run(&["--render", &clip_arg, "--out", "unused.png"]),
        "--out",
        "--render with --out",
    );

    // A decimal rate is rejected rather than approximated — 29.97 is 30000/1001,
    // and rounding it drifts the picture against its own soundtrack.
    let out = run(&["--render", &clip_arg, "--fps", "29.97"]);
    assert_failed_naming(&out, "--fps", "a decimal frame rate");
    assert!(
        stderr(&out).contains("30000/1001"),
        "the error must name the exact form to write:\n{}",
        stderr(&out)
    );

    // `--fps` outside the render mode would be accepted and ignored.
    assert_failed_naming(
        &run(&["--preset-file", SHIPPED_PRESET_FILE, "--fps", "30"]),
        "--fps only applies",
        "--fps without --render",
    );
}

/// **Phase 3's done-when.** For the same preset, clip and instant, the frame the
/// render mode hands its stream writer is **byte-identical** to the PNG
/// `shot --frame-at` writes.
///
/// This is the guard on **where the export tap sits**. The composite is
/// linear-light `Rgba16Float` until the tonemap (ADR-0046) and the display write
/// dithers in the *encoded* domain (ADR-0096); a tap upstream of either would
/// produce a file washed out against the app in a way that reads as an engine
/// bug and that no check confined to the render path could see. Byte-identity is
/// exact on purpose — a tolerance here would pass with the tap in the wrong
/// place.
///
/// It is asserted on the **RGB frame**, never on the wire bytes: Y4M cannot
/// carry RGB, and an 8-bit RGB->YUV conversion is not bijective, so a wire-level
/// version of this assertion could only ever be loosened until it passed. The
/// conversion is a separate, measured property (`the_colour_conversion_round_
/// trips_to_within_a_level`).
///
/// **The clip's sample rate is load-bearing.** [`HOP_SIZE`] samples at 30,720 Hz
/// is exactly 60 analysis hops a second, so at `--fps 60` output frame N and
/// analysis hop N are the same instant — and `Fps { 60, 1 }.dt()` is bit-for-bit
/// the fixed step every other capture path takes. At 48 kHz the two clocks are
/// 93.75 Hz and 60 Hz and no frame index lines up with a hop, which is the whole
/// reason they are separate clocks; the alignment is arranged here rather than
/// assumed anywhere in the code.
#[test]
fn a_rendered_frame_is_byte_identical_to_the_png_the_app_writes() {
    use rlx_core::dsp::HOP_SIZE;
    use rlx_core::preset::Preset;
    use rlx_core::render::Tier;
    use standalone::shot::render::{DEFAULT_FPS, render_frames};

    /// One analysis hop per output frame at 60 fps.
    const RATE: u32 = HOP_SIZE as u32 * 60;
    /// Well past the analyzer's 16-hop warm-up, so the frame carries real
    /// analysis rather than the silence both paths start in.
    const AT: u32 = 45;
    const W: u32 = 32;
    const H: u32 = 24;

    let clip = render_clip("tap.wav", RATE, 1.5);
    let png = scratch("render").join("tap.png");
    let _ = std::fs::remove_file(&png);

    // The app's own frame, through the CLI a preset author actually runs.
    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--audio",
        &clip.to_string_lossy(),
        "--frame-at",
        &AT.to_string(),
        "--out",
        &png.to_string_lossy(),
        "--size",
        &format!("{W}x{H}"),
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--frame-at failed\nstderr: {}",
        stderr(&out)
    );
    let expected = load_capture(&png);

    // The render mode's frame, taken where the stream writer takes it.
    let src = std::fs::read(&clip).unwrap_or_else(|e| panic!("read {}: {e}", clip.display()));
    let (pcm, format) = standalone::shot::wav::parse_wav_16bit(&src, "tap.wav")
        .unwrap_or_else(|e| panic!("the render fixture must parse: {e}"));
    let toml = std::fs::read_to_string(repo_root().join(SHIPPED_PRESET_FILE))
        .unwrap_or_else(|e| panic!("read {SHIPPED_PRESET_FILE}: {e}"));
    let preset = Preset::from_toml_str(&toml)
        .unwrap_or_else(|e| panic!("{SHIPPED_PRESET_FILE} must parse: {e}"));
    let name = preset.name.clone();
    let mut r = standalone::shot::renderer(W, H, vec![preset], Tier::Floor)
        .unwrap_or_else(|e| panic!("headless renderer: {e}"));

    let mut got: Option<CaptureImage> = None;
    render_frames(
        &mut r,
        &name,
        &pcm,
        format,
        DEFAULT_FPS,
        &mut |index, img| {
            if index == AT {
                got = Some(img.clone());
            }
            Ok(())
        },
    )
    .unwrap_or_else(|e| panic!("render: {e}"));
    let got = got.expect("frame 45 of a 90-frame render");

    assert_eq!(
        (got.width, got.height),
        (expected.width, expected.height),
        "the render and the PNG are not even the same size"
    );
    assert!(
        got.rgba == expected.rgba,
        "the rendered frame differs from the PNG the app writes at the same \
         instant — the export tap is not where the display write is (mean \
         difference {:.4}, ADR-0046 tonemap / ADR-0096 dither)",
        frame_diff(&got, &expected)
    );
}

/// A minimal reaction-diffusion preset, written by the test that uses it.
///
/// The family Plan 0099 measured as the worst case for per-pass retention —
/// twelve simulation sub-steps plus a present, thirteen passes a frame, against
/// single-pass worlds' one. The probe owns its subject rather than naming a
/// shipped preset: a memory assertion coupled to the content library breaks the
/// day a cohort retires the file, which is exactly how the tempo probe above
/// broke.
const RD_PROBE_SRC: &str = r#"
system = "reaction_diffusion"
name = "probe_render_memory"
[params]
feed = "0.036"
kill = "0.0645"
flow = "2.0"
inject = "beat"
"#;

/// **Phase 4's guard.** A render long enough to expose per-frame retention
/// completes, and reports its resident set.
///
/// Plan 0099 found the wall: a capture path that submitted without polling
/// retained per **pass**, so a thirteen-pass reaction-diffusion world held
/// 950 KB a frame and hit the allocator at ~4.4 GB. The render mode does not
/// inherit that — every frame goes through `capture_stream`, which reads back,
/// and the readback polls — but "does not inherit it" is a claim about a call
/// graph, and a future edit that submits its own passes here would break it
/// silently.
///
/// Six hundred frames rather than the done-when's 14,400: the defect grew
/// *linearly with frame count*, so 600 frames of it is ~570 MB and a ceiling
/// catches it, while a four-minute 1080p run is minutes of GPU time and belongs
/// in a hand-run measurement rather than in a suite that runs on every push. The
/// ceiling is deliberately generous — it is a tripwire for a defect of that
/// scale, not a budget, and the number that is actually judged is the one the
/// run prints.
#[test]
fn a_long_render_completes_and_reports_a_flat_resident_set() {
    /// Frames rendered — 10 s at 60 fps, and enough that a per-frame leak of the
    /// scale Plan 0099 measured would be hundreds of megabytes.
    const FRAMES: u32 = 600;
    /// Growth past which this is a leak. Charged from the **warm** reading, so
    /// the ~76 MB of pipeline compilation the first draw pays for is not in it —
    /// on this box the run after that step is flat to the megabyte across all
    /// nineteen remaining samples. Sixty-four megabytes is generous against a
    /// measured zero and still an order of magnitude under what Plan 0099's
    /// per-pass retention would show at this length.
    const CEILING_MB: f64 = 64.0;

    let dir = scratch("render-memory");
    let preset = dir.join("rd_probe.toml");
    std::fs::write(&preset, RD_PROBE_SRC).expect("write the probe preset");
    let clip = render_clip("long.wav", 48_000, FRAMES as f32 / 60.0);

    let out = run(&[
        "--preset-file",
        &preset.to_string_lossy(),
        "--render",
        &clip.to_string_lossy(),
        "--fps",
        "60",
        "--size",
        "64x48",
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "the long render failed\nstderr: {}",
        stderr(&out)
    );
    assert_eq!(
        y4m_frame_count(&out.stdout),
        FRAMES as usize,
        "the render did not reach the end of the clip"
    );

    // The resident-set line is the instrument; losing it would make the
    // done-when unmeasurable without saying so.
    let log = stderr(&out);
    let line = log
        .lines()
        .find(|l| l.starts_with("render: resident set"))
        .unwrap_or_else(|| panic!("no resident-set line in:\n{log}"));
    assert!(
        line.contains(&format!("across {FRAMES} frames")),
        "the line counts a different run: {line}"
    );

    // Sampled across the run and not merely at its ends: before-and-after alone
    // cannot tell a run that grew steadily from one that stepped once at
    // startup, which on this box is a 76 MB difference in what gets reported.
    let samples: usize = line
        .split(", ")
        .last()
        .and_then(|tail| tail.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no sample count in: {line}"));
    assert!(
        samples > 10,
        "the resident set was sampled {samples} times across {FRAMES} frames: {line}"
    );

    let growth: f64 = line
        .split("growth ")
        .nth(1)
        .and_then(|rest| rest.split(" MB").next())
        .and_then(|mb| mb.parse().ok())
        .unwrap_or_else(|| panic!("no growth figure in: {line}"));
    eprintln!("{line}");
    assert!(
        growth < CEILING_MB,
        "the resident set grew {growth:.1} MB over {FRAMES} frames — a render \
         that leaks is the same defect as a live session that leaks (NFR 12), \
         and Plan 0099's per-pass retention is what this looks like: {line}"
    );
}

/// `ffmpeg` on `PATH`, or `None` with a printed skip.
///
/// The encoder is a **documented prerequisite**, not a bundled component
/// (ADR-0114), so a runner without one is a legitimate skip in exactly the way a
/// runner without a GPU adapter is. The skip is keyed on the binary being
/// spawnable, so an `ffmpeg` that exists but fails is still a failure.
fn ffmpeg_on_path() -> bool {
    match Command::new("ffmpeg").arg("-version").output() {
        Ok(out) if out.status.success() => true,
        _ => {
            eprintln!("skipped: no `ffmpeg` on PATH (a documented prerequisite, ADR-0114)");
            false
        }
    }
}

/// **Phase 2's first done-when**: one command over a WAV produces a playable MP4
/// with audio.
///
/// "Playable" is checked with `ffprobe` where it exists — it ships beside
/// `ffmpeg` — and falls back to the container's own `ftyp` box otherwise.
/// ADR-0114 accepts that nothing here validates the encode itself; what is
/// asserted is that the mux happened and that both streams are in the file,
/// which is the failure a music video without music would be.
#[test]
fn one_command_over_a_wav_produces_an_mp4_with_audio() {
    if !ffmpeg_on_path() {
        return;
    }
    let clip = render_clip("encode.wav", 48_000, 1.0);
    let mp4 = scratch("render").join("out.mp4");
    let _ = std::fs::remove_file(&mp4);

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--render",
        &clip.to_string_lossy(),
        "--fps",
        "30",
        "--size",
        "64x48",
        "--ffmpeg",
        "ffmpeg",
        "--out",
        &mp4.to_string_lossy(),
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "the one-command render failed\nstderr: {}",
        stderr(&out)
    );
    // stdout stays empty on the encoder path — the frames went down the pipe.
    assert!(
        out.stdout.is_empty(),
        "the frame stream leaked to stdout instead of the encoder"
    );
    // The generated command is echoed, so adapting it starts from what ran.
    assert!(
        stderr(&out).contains("-f yuv4mpegpipe"),
        "the canonical invocation was not printed:\n{}",
        stderr(&out)
    );

    let bytes = std::fs::read(&mp4).unwrap_or_else(|e| panic!("read {}: {e}", mp4.display()));
    assert!(bytes.len() > 1024, "the MP4 is {} bytes", bytes.len());
    assert_eq!(&bytes[4..8], b"ftyp", "not an MP4 container");

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(&mp4)
        .output();
    match probe {
        Ok(p) if p.status.success() => {
            let streams = String::from_utf8_lossy(&p.stdout).into_owned();
            assert!(streams.contains("video"), "no video stream: {streams}");
            assert!(
                streams.contains("audio"),
                "the source WAV was not muxed in — a music video with no music: {streams}"
            );
        }
        _ => eprintln!("note: no `ffprobe`; checked the container header only"),
    }
}

/// **The four colour tags, read back off the artifact rather than off the
/// command line.**
///
/// `docs/capturing.md` calls these *"the half most likely to ship wrong"* — an
/// untagged file is one a player expands from studio swing and shows washed
/// out — and until this assertion existed the only check was that the flags
/// appeared in the generated arguments. Two of them did not survive: a file
/// written with `-colorspace bt709 -color_primaries bt709 -color_trc bt709`
/// reads back `bt709/unknown/unknown`, because the libx264 path honours the
/// first and drops the other two. They are set on x264 directly as well, and
/// this is what holds that.
///
/// Gated on `ffmpeg_on_path()` and on an adapter, so it is a no-op where either
/// is missing — which is most CI runners, and is why it cannot be the only
/// guard. The unit test over `ffmpeg_args` is the half that runs everywhere;
/// this is the half that convicts.
#[test]
fn the_four_colour_tags_survive_into_the_container() {
    if !ffmpeg_on_path() {
        return;
    }
    let clip = render_clip("colour-tags.wav", 48_000, 0.5);
    let mp4 = scratch("colour-tags").join("tagged.mp4");
    let _ = std::fs::remove_file(&mp4);

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--render",
        &clip.to_string_lossy(),
        "--fps",
        "30",
        "--size",
        "64x48",
        "--ffmpeg",
        "ffmpeg",
        "--out",
        &mp4.to_string_lossy(),
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "the encode failed\nstderr: {}",
        stderr(&out)
    );

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=color_range,color_space,color_primaries,color_transfer",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&mp4)
        .output();
    let probe = match probe {
        Ok(p) if p.status.success() => String::from_utf8_lossy(&p.stdout).into_owned(),
        _ => {
            eprintln!("skipped: `ffprobe` did not run, so the container was not read back");
            return;
        }
    };
    // `unknown` is what a dropped tag reads as, and naming it separately from the
    // value keeps the failure legible: a tag that arrives with the *wrong* value
    // is a different defect from one that never arrived.
    for (key, value) in [
        ("color_range", "pc"),
        ("color_space", "bt709"),
        ("color_primaries", "bt709"),
        ("color_transfer", "bt709"),
    ] {
        let line = format!("{key}={value}");
        assert!(
            probe.contains(&line),
            "the container does not carry `{line}` — a tag on the command line \
             that does not reach the file is a guarantee stated and not held\n\
             ffprobe:\n{probe}"
        );
    }
}

/// **Phase 2's second done-when**: an encoder that dies mid-render makes `shot`
/// exit non-zero **with the encoder's own message**.
///
/// The stand-in encoder is `shot` itself handed `ffmpeg`'s arguments: it rejects
/// the first one and exits 1 with its own text on stderr. That is a faithful
/// model of the real failure — a child that dies while frames are still being
/// written — and it needs no `ffmpeg` and no OS-specific way to kill a process.
///
/// A frame here is 320x240x3 = 230 KB against a ~64 KB pipe buffer, so the write
/// genuinely breaks mid-stream rather than fitting into the buffer and only
/// surfacing at exit. What must *not* happen is `shot` reporting its own broken
/// pipe: that is the mystery `EPIPE` this path is most likely to produce.
#[test]
fn an_encoder_that_dies_reports_the_encoders_own_failure() {
    let clip = render_clip("broken-pipe.wav", 48_000, 1.0);
    let fake = shot_bin();

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--render",
        &clip.to_string_lossy(),
        "--fps",
        "30",
        "--size",
        "320x240",
        "--ffmpeg",
        &fake.to_string_lossy(),
        "--out",
        "unused.mp4",
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        !out.status.success(),
        "a dead encoder must exit non-zero\nstderr: {}",
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(
        err.contains("the encoder exited with"),
        "the failure is attributed to the encoder:\n{err}"
    );
    // ...and quotes what it actually said, rather than our own write error.
    assert!(
        err.contains("unknown argument"),
        "the encoder's own message is missing from the report:\n{err}"
    );
}

/// A missing encoder is a named error naming the flag, and never a silent
/// fallback to something else (ADR-0114). GPU-free.
#[test]
fn a_missing_encoder_names_the_flag_rather_than_falling_back() {
    let clip = render_clip("no-encoder.wav", 48_000, 0.5);
    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--render",
        &clip.to_string_lossy(),
        "--ffmpeg",
        "no_such_encoder_binary",
        "--out",
        "unused.mp4",
    ]);
    assert_failed_naming(&out, "--ffmpeg", "a missing encoder");
    // The encoder is spawned before the renderer is built, so a missing one
    // fails immediately rather than after GPU initialization.
    assert!(
        !stderr(&out).contains("adapter"),
        "a missing encoder should not have reached the renderer:\n{}",
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(
        err.contains("no encoder ships"),
        "the message must say there is no fallback:\n{err}"
    );

    // `--ffmpeg` has nowhere to write without `--out`, and `--out` alone has
    // nothing to name — both are errors rather than a guessed destination.
    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--render",
            &clip.to_string_lossy(),
            "--ffmpeg",
            "ffmpeg",
        ]),
        "--out",
        "--ffmpeg without --out",
    );
    assert_failed_naming(
        &run(&["--preset-file", SHIPPED_PRESET_FILE, "--ffmpeg", "ffmpeg"]),
        "--ffmpeg only applies",
        "--ffmpeg without --render",
    );

    // `--crf` is `--ffmpeg`'s companion and has the same two ways to be
    // meaningless: without an encoder there is no command line for it to appear
    // in, and outside the render mode there is no encoder at all. Both were
    // rejected in the parser and asserted nowhere.
    assert_failed_naming(
        &run(&[
            "--preset-file",
            SHIPPED_PRESET_FILE,
            "--render",
            &clip.to_string_lossy(),
            "--crf",
            "20",
        ]),
        "--crf",
        "--crf without --ffmpeg",
    );
    assert_failed_naming(
        &run(&["--preset-file", SHIPPED_PRESET_FILE, "--crf", "20"]),
        "--crf only applies",
        "--crf without --render",
    );
}

/// **The spend-nothing ordering, held by something other than the line order in
/// `render::run()`.**
///
/// Plan 0139 exists to stop a rejected preset name from leaving a valid,
/// playable, audio-only MP4 at the destination: `ffmpeg` exits 0 on a frame
/// stream that never carried a frame. Its repair was to resolve the name before
/// the encoder is spawned and before a device is built — and `resolve_preset`
/// returns the same answer from either side of that call, so the only thing
/// holding the ordering was the order of two lines. A refactor of `run()`
/// reintroduces the artifact **with a green suite**.
///
/// The reproduction is a *filename* against a roster keyed on `name`, which is
/// the confusion that produced the original report: `attractor_leviathan.toml`
/// carries `name = "Leviathan"`, so the file's stem is not a key.
///
/// `--ffmpeg no_such_encoder_binary` is what makes this need no encoder: if the
/// ordering ever regresses, the spawn fails first and the assertions below name
/// the flag instead of the roster. **That is why stderr must not contain
/// `--ffmpeg`** — it is the difference between "validated first" and "tried to
/// spawn and failed", and it is the assertion that keeps the test's mechanism
/// legible if a missing encoder ever stops being fatal.
#[test]
fn a_render_that_cannot_name_its_preset_spends_nothing() {
    let clip = render_clip("unspent.wav", 48_000, 0.5);
    let out_path = scratch("render-unspent").join("must-not-be-written.mp4");
    let _ = std::fs::remove_file(&out_path);

    let out = run(&[
        "--render",
        &clip.to_string_lossy(),
        // Pin the roster to this repository. Without it the resolver reads the
        // per-user preset directory, which outranks the embedded set and is
        // seeded write-if-absent - so a preset renamed under `presets/` leaves a
        // stale copy there and the two key assertions below keep passing off it.
        "--presets",
        "presets",
        "--preset",
        "attractor_leviathan",
        "--ffmpeg",
        "no_such_encoder_binary",
        "--out",
        &out_path.to_string_lossy(),
    ]);
    assert_failed_naming(
        &out,
        "unknown preset `attractor_leviathan`",
        "a filename where a preset name belongs",
    );

    let err = stderr(&out);
    // The roster's keys are printed, because the name is the preset's `name`
    // field and not its filename. Two shipped keys stand in for the list.
    assert!(
        err.contains("Leviathan") && err.contains(SHIPPED_PRESET_NAME),
        "the rejection must name the roster's keys:\n{err}"
    );
    // Nothing was spawned...
    assert!(
        !err.contains("--ffmpeg"),
        "the encoder was reached before the name was resolved:\n{err}"
    );
    // ...no device was built...
    assert!(
        !err.contains("adapter"),
        "a rejected preset name should not have reached the renderer:\n{err}"
    );
    // ...and the destination is untouched. This is the clause that fails when
    // the ordering regresses: the file the original defect left behind was a
    // valid 262-byte MP4, indistinguishable at a glance from a short render.
    assert!(
        !out_path.exists(),
        "a rejected render left a file at {}",
        out_path.display()
    );
}

/// Every flag name `parse_args` compares an argument against, read out of the
/// example's own source.
///
/// A flag literal is the **whole** string — `"--preset"` — which is what
/// separates a match arm from prose that mentions a flag. Every error message in
/// this CLI opens with the flag it is about (`"--frames expects a positive
/// integer"`), and the usage text is one long literal whose lines start with a
/// flag name; neither is a name the parser accepts, and requiring the closing
/// quote immediately after the name excludes both.
fn parser_flag_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(offset) = source[i..].find("\"--") {
        // Past the opening quote; the name itself starts at the dashes.
        let start = i + offset + 1;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end] == b'-'
                || bytes[end].is_ascii_lowercase()
                || bytes[end].is_ascii_digit())
        {
            end += 1;
        }
        // A bare `"--"` names nothing, and a name is only a name when the string
        // ends right after it.
        if end > start + 2 && end < bytes.len() && bytes[end] == b'"' {
            let name = source[start..end].to_owned();
            if !found.contains(&name) {
                found.push(name);
            }
        }
        i = start + 2;
    }
    found
}

/// **The drift gate `ritmolux` has and `shot` did not.** ADR-0148 gave the app's
/// roster a test so a flag cannot be added to a scanner and left out of
/// `--help`. `shot` has the same failure mode: its flags are matched in one arm
/// each and re-typed by hand into `print_usage()` and again into
/// `docs/capturing.md`'s table.
///
/// That matters on this CLI specifically — it is the one the `preset-author`
/// lane drives, and `CLAUDE.md` routes that lane to `docs/capturing.md`, whose
/// table is transcribed from the usage text. A flag that exists and is
/// undocumented is invisible to the only consumer that needs it.
///
/// One-directional by construction, exactly as ADR-0148's is: it cannot assert
/// that every line of the usage text is still reachable, so a retired flag can
/// linger there. The shared roster type both binaries would construct from is
/// deliberately not built — two CLIs do not pay for it, and this buys the
/// property that was actually missing.
#[test]
fn the_usage_text_names_every_flag_the_parser_accepts() {
    let source = include_str!("../examples/shot.rs");
    let flags = parser_flag_literals(source);
    // A lexer that silently stopped matching would make this pass by finding
    // nothing, which is the one way a drift gate fails quietly.
    assert!(
        flags.len() >= 20,
        "the scan found only {} flag literals in examples/shot.rs; it has stopped \
         reading the source: {flags:?}",
        flags.len()
    );

    let out = run(&["--help"]);
    assert!(
        out.status.success(),
        "--help must exit 0, got {}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    // The usage goes to stderr, because stdout is the frame stream on the render
    // path and a mode that printed help to stdout would be inconsistent with it.
    let usage = stderr(&out);
    for flag in &flags {
        assert!(
            usage.contains(flag.as_str()),
            "`{flag}` is a match arm in examples/shot.rs and --help does not \
             mention it, so the binary accepts a flag its own usage text hides\n\
             usage:\n{usage}"
        );
    }
    // `-h` is a single-dash synonym the extraction above cannot see, so it is
    // named here rather than silently uncovered.
    assert!(
        usage.contains("-h"),
        "--help does not name its own synonym:\n{usage}"
    );
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
/// `--report`'s own window is shorter, and the fall does **not**
/// therefore read clamped — nothing clamps; `frames_to_settle` returns a
/// plausible smaller number (Plan 0038 Phase 8 corrected the reverse
/// claim, which had been written here). That is why the cell carries a
/// `+` here, which this test also asserts reaches both presentations. The
/// separation is what matters either way.
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
            // `fixture_easing_*` is longer than the report's name column, and
            // an over-long name is elided in the MIDDLE (`fixture~scalar`)
            // rather than truncated (Plan 0121 Phase 2), so the row is found by
            // the head the elision keeps.
            .find(|l| l.trim_start().starts_with("fixture"))
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
