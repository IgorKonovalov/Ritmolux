//! `shot` — headless capture / visual-QA CLI (Plan 0013). Dev/agent tooling: it
//! renders presets with **no window** and writes PNGs the agent can Read, or a
//! metrics report (text / JSON) the agent can parse. It links the same
//! `lmv-core` the app does; `image` (a dev-dependency, ADR-0011) only encodes
//! and tiles, so the shipped `lmv.exe` is untouched.
//!
//! What is left here is what an `examples/` target has to own: argument parsing,
//! GPU capture, and file I/O. Every **pure** helper lives in
//! [`standalone::shot`] instead, because `#[test]` in an example does not run
//! under `cargo test` (Plan 0031 Phase 1) — so the WAV parse, the JSON escaping,
//! the filmstrip index/layout math and the glyph table are unit-tested there.
//!
//! Run: `cargo run -p standalone --example shot -- --preset Aurora --out shot.png`
//!
//! Flags:
//!   --preset <name>          single-shot the named preset
//!   --presets <dir>          load the library from <dir> (e.g. the repo's presets/)
//!   --preset-file <path>     load exactly one preset from <path>
//!   --set k=v,...            constant stimulus (bass/mid/treb/onset/beat/bar/
//!                            tempo/novelty) — a *held* value, not a transient
//!   --frames <N>             frames to advance before capture (default 120)
//!   --size <WxH>             render size (default 1280x720)
//!   --out <path>             output PNG (single shot) or dir/file (--all)
//!   --all                    contact sheet of every preset (needs --out)
//!   --report [family=<sys>]  per-family reactivity / animation / distinctness
//!            [--json]        emit JSON instead of a text table
//!   --signal <kind:param>    synth audio filmstrip (click:120, bass:60, ...)
//!   --audio <clip.wav>       filmstrip from a 16-bit PCM WAV
//!   --strip <N>              frames tiled along the audio (default 8)
//!
//! Which preset library is used, highest precedence first: `--preset-file`,
//! `--presets`, the `LMV_PRESET_DIR` override, the per-user preset directory,
//! the embedded defaults. The app resolves the last three through the very same
//! `standalone` library function (ADR-0014), so `LMV_PRESET_DIR=./presets` points
//! the running window and a headless capture at one editable folder.
//!
//! Exit code is non-zero with a message on any bad argument or failure.

use std::path::{Path, PathBuf};

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, SPECTRUM_BINS};
use lmv_core::preset::{Preset, SystemKind, default_presets, load_dir};
use lmv_core::render::metrics::{coverage, frame_diff, quadrant_spread, struct_diff};
use lmv_core::render::{CaptureImage, HeadlessOptions, Renderer};
use standalone::shot::args::{BandLevels, apply_set, band_levels, parse_size, synth_signal};
use standalone::shot::film::{StripLayout, filmstrip_indices, filmstrip_layout};
use standalone::shot::glyph::{GLYPH_ADVANCE, GLYPH_COLS, glyph_for};
use standalone::shot::json::{json_matrix, json_string, num};
use standalone::shot::wav::parse_wav_16bit;
use standalone::{PRESET_DIR_ENV, resolve_preset_dir};

fn main() {
    if let Err(msg) = run() {
        eprintln!("shot: {msg}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

enum Mode {
    Shot,
    All,
    Report,
}

struct Args {
    mode: Mode,
    preset: Option<String>,
    /// `--presets <dir>`: load the library from here instead of the resolved
    /// preset directory.
    presets: Option<PathBuf>,
    /// `--preset-file <path>`: a one-entry roster read from this file. Beats
    /// `--presets`.
    preset_file: Option<PathBuf>,
    stimulus: AnalysisFrame,
    frames: u32,
    width: u32,
    height: u32,
    out: Option<PathBuf>,
    family: Option<SystemKind>,
    json: bool,
    signal: Option<String>,
    audio: Option<PathBuf>,
    strip: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            mode: Mode::Shot,
            preset: None,
            presets: None,
            preset_file: None,
            stimulus: AnalysisFrame::default(),
            frames: 120,
            width: 1280,
            height: 720,
            out: None,
            family: None,
            json: false,
            signal: None,
            audio: None,
            strip: 8,
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--preset" => args.preset = Some(next_value(&mut it, "--preset")?),
            "--presets" => args.presets = Some(PathBuf::from(next_value(&mut it, "--presets")?)),
            "--preset-file" => {
                args.preset_file = Some(PathBuf::from(next_value(&mut it, "--preset-file")?));
            }
            "--set" => apply_set(&mut args.stimulus, &next_value(&mut it, "--set")?)?,
            "--frames" => {
                args.frames = next_value(&mut it, "--frames")?
                    .parse()
                    .map_err(|_| "--frames expects a positive integer".to_string())?;
            }
            "--size" => {
                let (w, h) = parse_size(&next_value(&mut it, "--size")?)?;
                args.width = w;
                args.height = h;
            }
            "--out" => args.out = Some(PathBuf::from(next_value(&mut it, "--out")?)),
            "--all" => args.mode = Mode::All,
            "--report" => args.mode = Mode::Report,
            "--json" => args.json = true,
            "--signal" => args.signal = Some(next_value(&mut it, "--signal")?),
            "--audio" => args.audio = Some(PathBuf::from(next_value(&mut it, "--audio")?)),
            "--strip" => {
                args.strip = next_value(&mut it, "--strip")?
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or("--strip expects a positive integer")?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other if other.starts_with("family=") => {
                args.family = Some(parse_system(other.trim_start_matches("family="))?);
            }
            other => return Err(format!("unknown argument `{other}` (try --help)")),
        }
    }
    Ok(args)
}

fn next_value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_system(name: &str) -> Result<SystemKind, String> {
    // The name↔kind mapping is single-sourced on SystemKind in core; keep only
    // shot's friendly error text here.
    SystemKind::from_name(name).ok_or_else(|| {
        // The list of legal names comes off the roster too, so a new system
        // cannot ship missing from the error text (it used to be hand-written).
        let known: Vec<&str> = SystemKind::ALL.iter().map(|k| k.as_str()).collect();
        format!("unknown family `{name}` ({})", known.join(" | "))
    })
}

fn print_usage() {
    eprintln!(
        "shot — headless capture / visual-QA (Plan 0013)\n\
         \n\
         --preset <name>            single-shot the named preset (needs --out)\n\
         --presets <dir>            library directory (beats LMV_PRESET_DIR)\n\
         --preset-file <path>       one preset from a file (beats --presets)\n\
         --set k=v,...              bass,mid,treb,onset,bar,beat,tempo,novelty\n\
         (each HELD for every captured frame - see docs/capturing.md)\n\
         --frames <N>               frames before capture (default 120)\n\
         --size <WxH>               render size (default 1280x720)\n\
         --out <path>               PNG path (shot) or dir/file (--all)\n\
         --all                      contact sheet of every preset (needs --out)\n\
         --report [family=<sys>]    metrics table (fragment_field | swarm)\n\
         --json                     emit the report as JSON\n\
         --signal <kind:param>      synth audio filmstrip: click:120 bass:60\n\
                                    treble:10000 noise:7 chord (needs --out)\n\
         --audio <clip.wav>         filmstrip from a 16-bit PCM WAV (needs --out)\n\
         --strip <N>                frames tiled along the audio (default 8)"
    );
}

// ---------------------------------------------------------------------------
// Preset library + renderer
// ---------------------------------------------------------------------------

/// Load the preset library, highest precedence first: `--preset-file` (a
/// one-entry roster), `--presets <dir>`, the shared resolver (which honors
/// `LMV_PRESET_DIR`, else the per-user directory), the embedded defaults.
/// Returns the presets and a label naming which source won.
///
/// The two explicit flags are errors when they yield nothing — an agent that
/// asked for a specific folder or file wants a non-zero exit, not a silent
/// capture of some other library. The resolved directory keeps degrading to the
/// embedded defaults, exactly as the app does (NFR 10).
fn load_library(args: &Args) -> Result<(Vec<Preset>, String), String> {
    if let Some(path) = &args.preset_file {
        let src = std::fs::read_to_string(path)
            .map_err(|e| format!("--preset-file {}: {e}", path.display()))?;
        let preset = Preset::from_toml_str(&src)
            .map_err(|e| format!("--preset-file {}: {e}", path.display()))?;
        return Ok((vec![preset], format!("--preset-file {}", path.display())));
    }

    if let Some(dir) = &args.presets {
        let report = load_dir(dir);
        report_errors(&report);
        if report.presets.is_empty() {
            return Err(format!("--presets {}: no valid presets", dir.display()));
        }
        return Ok((report.presets, format!("--presets {}", dir.display())));
    }

    let resolved = resolve_preset_dir();
    if let Some(dir) = resolved.path() {
        let report = load_dir(dir);
        report_errors(&report);
        if !report.presets.is_empty() {
            let label = if resolved.is_override() {
                format!("{PRESET_DIR_ENV} {}", dir.display())
            } else {
                format!("on-disk {}", dir.display())
            };
            return Ok((report.presets, label));
        }
    }
    Ok((default_presets(), "embedded defaults".to_string()))
}

/// Surface malformed files on stderr — a preset being silently absent from a
/// capture is the confusing failure this CLI exists to avoid.
fn report_errors(report: &lmv_core::preset::LoadReport) {
    for (path, err) in &report.errors {
        eprintln!("shot: preset {}: {err}", path.display());
    }
}

/// `(name, system)` pairs for the loaded library, in roster order.
fn preset_meta(presets: &[Preset]) -> Vec<(String, SystemKind)> {
    presets.iter().map(|p| (p.name.clone(), p.system)).collect()
}

/// A headless renderer over `presets`, using the real GPU at full quality (the
/// CLI wants speed and true output, not the tests' software reproducibility).
fn renderer(width: u32, height: u32, presets: Vec<Preset>) -> Result<Renderer, String> {
    let mut r = Renderer::new_headless(HeadlessOptions {
        width,
        height,
        prefer_software: false,
    })
    .map_err(|e| format!("headless renderer: {e}"))?;
    r.set_presets(presets);
    Ok(r)
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let (presets, source) = load_library(&args)?;
    // An audio source (synth signal or WAV) takes precedence — it drives a
    // filmstrip regardless of the shot/all/report mode default.
    if args.signal.is_some() || args.audio.is_some() {
        return filmstrip(args, presets, &source);
    }
    match args.mode {
        Mode::Shot => shot(args, presets, &source),
        Mode::All => contact_sheet(args, presets, &source),
        Mode::Report => report(args, presets, &source),
    }
}

fn shot(args: Args, presets: Vec<Preset>, source: &str) -> Result<(), String> {
    // A one-entry roster (`--preset-file`, or a folder holding a single preset)
    // names itself, so `--preset` is only required when there is a choice.
    let name = match (&args.preset, presets.as_slice()) {
        (Some(name), _) => name.clone(),
        (None, [only]) => only.name.clone(),
        (None, _) => return Err("--preset <name> is required for a single shot".to_string()),
    };
    let out = args.out.clone().ok_or("--out <path> is required")?;
    let mut r = renderer(args.width, args.height, presets)?;
    let img = r
        .capture_preset(&name, &args.stimulus, args.frames)
        .map_err(|e| format!("capture `{name}`: {e}"))?;
    save_png(&img, &out)?;
    println!(
        "wrote {} ({}x{}, preset {name}, {} frames) [{source}]",
        out.display(),
        img.width,
        img.height,
        args.frames
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// PNG + contact sheet
// ---------------------------------------------------------------------------

fn save_png(img: &CaptureImage, path: &Path) -> Result<(), String> {
    let buffer = to_rgba(img)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    buffer
        .save(path)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn to_rgba(img: &CaptureImage) -> Result<image::RgbaImage, String> {
    image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone())
        .ok_or_else(|| "capture buffer does not match its dimensions".to_string())
}

fn contact_sheet(args: Args, presets: Vec<Preset>, source: &str) -> Result<(), String> {
    let out = args.out.clone().ok_or("--all needs --out <dir-or-file>")?;
    let meta = preset_meta(&presets);
    if meta.is_empty() {
        return Err("no presets to tile".to_string());
    }
    let mut r = renderer(args.width, args.height, presets)?;

    // Layout: a near-square grid of fixed-width thumbnails with a label strip.
    const THUMB_W: u32 = 320;
    const PAD: u32 = 8;
    const LABEL_H: u32 = 18;
    let thumb_h = (THUMB_W * args.height / args.width).max(1);
    let cols = (meta.len() as f64).sqrt().ceil() as u32;
    let rows = meta.len().div_ceil(cols as usize) as u32;
    let cell_w = THUMB_W + PAD;
    let cell_h = thumb_h + LABEL_H + PAD;
    let canvas_w = cols * cell_w + PAD;
    let canvas_h = rows * cell_h + PAD;

    let mut canvas =
        image::RgbaImage::from_pixel(canvas_w, canvas_h, image::Rgba([18, 18, 22, 255]));

    for (i, (name, _system)) in meta.iter().enumerate() {
        let img = r
            .capture_preset(name, &args.stimulus, args.frames)
            .map_err(|e| format!("capture `{name}`: {e}"))?;
        let full = to_rgba(&img)?;
        let thumb = image::imageops::resize(
            &full,
            THUMB_W,
            thumb_h,
            image::imageops::FilterType::Triangle,
        );
        let col = i as u32 % cols;
        let row = i as u32 / cols;
        let x = PAD + col * cell_w;
        let y = PAD + row * cell_h;
        image::imageops::overlay(&mut canvas, &thumb, x as i64, y as i64);
        draw_label(
            &mut canvas,
            x,
            y + thumb_h + 3,
            name,
            [235, 235, 240, 255],
            2,
        );
    }

    let path = contact_sheet_path(&out);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    canvas
        .save(&path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    println!(
        "wrote {} ({} presets, {cols}x{rows} grid) [{source}]",
        path.display(),
        meta.len()
    );
    Ok(())
}

/// A `.png` `--out` is used verbatim; anything else is treated as a directory
/// and the sheet lands at `<out>/contact-sheet.png`.
fn contact_sheet_path(out: &Path) -> PathBuf {
    if out
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        out.to_path_buf()
    } else {
        out.join("contact-sheet.png")
    }
}

// ---------------------------------------------------------------------------
// Audio filmstrips (--signal / --audio)
// ---------------------------------------------------------------------------

fn filmstrip(args: Args, presets: Vec<Preset>, source: &str) -> Result<(), String> {
    let out = args
        .out
        .clone()
        .ok_or("--signal/--audio needs --out <path>")?;

    let (pcm, format, label) = match (&args.signal, &args.audio) {
        (Some(spec), _) => {
            let (pcm, fmt) = synth_signal(spec)?;
            (pcm, fmt, format!("signal {spec}"))
        }
        (None, Some(path)) => {
            let (pcm, fmt) = read_wav_16bit(path)?;
            (pcm, fmt, format!("audio {}", path.display()))
        }
        (None, None) => return Err("no --signal or --audio given".to_string()),
    };

    let meta = preset_meta(&presets);
    let name = args
        .preset
        .clone()
        .or_else(|| meta.first().map(|(n, _)| n.clone()))
        .ok_or("no preset available to render")?;

    let mut r = renderer(args.width, args.height, presets)?;
    let at = filmstrip_indices(pcm.len(), format, args.strip)?;
    let frames = r
        .capture_audio(&name, &pcm, format, &at)
        .map_err(|e| format!("capture audio: {e}"))?;

    let strip = tile_filmstrip(&frames)?;
    save_image(&strip, &out)?;
    // Printed unconditionally rather than behind a flag: an author who does not
    // already know that `--set` magnitudes are unlike real levels is exactly the
    // one who will never pass the flag.
    print_band_levels(&band_levels(&pcm, format)?);
    println!(
        "wrote {} ({} frames, preset {name}, {label}) [{source}]",
        out.display(),
        frames.len(),
    );
    Ok(())
}

/// Report what the analyzer derived from this clip, so "what does real material
/// actually produce" is answered with numbers instead of a guess.
fn print_band_levels(levels: &BandLevels) {
    println!(
        "audio levels over {} analysis hops (past warm-up) — calibrate gains against these, \
         not against --set magnitudes:",
        levels.hops
    );
    println!("  {:<5} {:>8} {:>8} {:>8}", "band", "min", "mean", "max");
    for (name, band) in [
        ("bass", levels.bass),
        ("mid", levels.mid),
        ("treb", levels.treb),
    ] {
        println!(
            "  {name:<5} {:>8.3} {:>8.3} {:>8.3}",
            band.min, band.mean, band.max
        );
    }
}

/// Read a 16-bit-PCM WAV off disk. The parse itself is
/// [`standalone::shot::wav`]; only the file read (and its path-labelled error)
/// belongs to the CLI.
fn read_wav_16bit(path: &Path) -> Result<(Vec<f32>, AudioFormat), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse_wav_16bit(&bytes, &path.display().to_string())
}

/// Tile the captured frames left-to-right into one filmstrip, each scaled to a
/// fixed height. The arithmetic is [`filmstrip_layout`]; this is the blit.
fn tile_filmstrip(frames: &[CaptureImage]) -> Result<image::RgbaImage, String> {
    let (frame_w, frame_h) = frames.first().map_or((1, 1), |f| (f.width, f.height));
    let layout: StripLayout = filmstrip_layout(frame_w, frame_h, frames.len())?;
    let mut canvas = image::RgbaImage::from_pixel(
        layout.canvas_w,
        layout.canvas_h,
        image::Rgba([18, 18, 22, 255]),
    );
    for (i, frame) in frames.iter().enumerate() {
        let full = to_rgba(frame)?;
        let thumb = image::imageops::resize(
            &full,
            layout.thumb_w,
            layout.thumb_h,
            image::imageops::FilterType::Triangle,
        );
        image::imageops::overlay(
            &mut canvas,
            &thumb,
            layout.x_of(i) as i64,
            layout.pad as i64,
        );
    }
    Ok(canvas)
}

/// Save a prepared `RgbaImage` to `path`, creating parent dirs.
fn save_image(img: &image::RgbaImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    img.save(path)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Report render size — small; the metrics don't need resolution.
const REPORT_SIZE: u32 = 192;
const REPORT_FRAMES: u32 = 24;
const REPORT_FRAMES_LATE: u32 = 48;
const NEAR_DUP_STRUCT: f32 = 0.08;
const COVERAGE_EPS: u8 = 10;

struct PresetReport {
    name: String,
    reactivity: [f32; 4], // bass, mid, treb, onset
    animation: f32,
    coverage: f32,
}

struct FamilyReport {
    system: SystemKind,
    presets: Vec<PresetReport>,
    pixel: Vec<Vec<f32>>,
    shape: Vec<Vec<f32>>,
    near_dups: Vec<(String, String)>,
}

fn report(args: Args, presets: Vec<Preset>, source: &str) -> Result<(), String> {
    let meta = preset_meta(&presets);
    let mut r = renderer(REPORT_SIZE, REPORT_SIZE, presets)?;

    // The roster, not a copy of it: a hand-maintained list here silently omitted
    // a new system from the report instead of failing to build.
    let mut reports = Vec::new();
    for system in SystemKind::ALL {
        if args.family.is_some_and(|f| f != system) {
            continue;
        }
        let names: Vec<String> = meta
            .iter()
            .filter(|(_, s)| *s == system)
            .map(|(n, _)| n.clone())
            .collect();
        if names.is_empty() {
            continue;
        }
        reports.push(build_family_report(&mut r, system, &names)?);
    }

    if args.json {
        print!("{}", render_json(source, &reports));
    } else {
        print_text_report(source, &reports);
    }
    Ok(())
}

fn build_family_report(
    r: &mut Renderer,
    system: SystemKind,
    names: &[String],
) -> Result<FamilyReport, String> {
    let silent = AnalysisFrame::default();
    let loud = loud_frame();
    let bands = band_stimuli();

    let mut presets = Vec::new();
    let mut fixed_caps = Vec::new();
    for name in names {
        let base = capture(r, name, &silent, REPORT_FRAMES)?;
        let mut reactivity = [0.0f32; 4];
        for (i, frame) in bands.iter().enumerate() {
            let lit = capture(r, name, frame, REPORT_FRAMES)?;
            reactivity[i] = frame_diff(&base, &lit);
        }
        let late = capture(r, name, &silent, REPORT_FRAMES_LATE)?;
        let animation = frame_diff(&base, &late);

        let fixed = capture(r, name, &loud, REPORT_FRAMES_LATE)?;
        let bg = corner(&fixed);
        let cov = coverage(&fixed, bg, COVERAGE_EPS);
        // Belt-and-braces "not a dot" note folded into coverage via spread:
        // a single-quadrant frame is suspicious even at decent coverage.
        let _spread = quadrant_spread(&fixed, bg, COVERAGE_EPS);

        presets.push(PresetReport {
            name: name.clone(),
            reactivity,
            animation,
            coverage: cov,
        });
        fixed_caps.push(fixed);
    }

    // Pairwise pixel + shape matrices over the fixed-frame captures.
    let n = fixed_caps.len();
    let mut pixel = vec![vec![0.0f32; n]; n];
    let mut shape = vec![vec![0.0f32; n]; n];
    let mut near_dups = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let pd = frame_diff(&fixed_caps[i], &fixed_caps[j]);
            let sd = struct_diff(&fixed_caps[i], &fixed_caps[j]);
            pixel[i][j] = pd;
            shape[i][j] = sd;
            if i < j && sd < NEAR_DUP_STRUCT {
                near_dups.push((names[i].clone(), names[j].clone()));
            }
        }
    }

    Ok(FamilyReport {
        system,
        presets,
        pixel,
        shape,
        near_dups,
    })
}

fn capture(
    r: &mut Renderer,
    name: &str,
    frame: &AnalysisFrame,
    frames: u32,
) -> Result<CaptureImage, String> {
    r.capture_preset(name, frame, frames)
        .map_err(|e| format!("capture `{name}`: {e}"))
}

/// The log-band index ranges each named band roughly occupies, so a report
/// stimulus lights the part of the `spectrum` array its own scalar summarises.
/// Mirrors `core/tests/reactivity.rs`: a frame claiming `bass = 1.0` over 64
/// silent log-bands is not a frame any audio could produce, and under it a
/// preset reading the array through `bin()` — or the whole `spectrum` system —
/// would correctly draw nothing, so the report would be scoring the fixture
/// rather than the preset.
///
/// **Approximate**, derived from the ~20 Hz-Nyquist log spacing and the
/// ~250 Hz / ~4 kHz band edges. They only need to be *distinct* regions for the
/// differential columns to mean anything; nothing here is a contract with the
/// DSP's exact edges.
const BASS_BANDS: std::ops::Range<usize> = 0..22;
const MID_BANDS: std::ops::Range<usize> = 22..48;
const TREB_BANDS: std::ops::Range<usize> = 48..64;

/// A held frame with one scalar band up and the matching slice of the log
/// spectrum lit.
fn band_stimulus(
    scalar: impl Fn(&mut AnalysisFrame),
    bands: std::ops::Range<usize>,
) -> AnalysisFrame {
    let mut frame = AnalysisFrame::default();
    scalar(&mut frame);
    for band in frame.spectrum.iter_mut().take(bands.end).skip(bands.start) {
        *band = 1.0;
    }
    frame
}

fn band_stimuli() -> [AnalysisFrame; 4] {
    [
        band_stimulus(|f| f.bass = 1.0, BASS_BANDS),
        band_stimulus(|f| f.mid = 1.0, MID_BANDS),
        band_stimulus(|f| f.treb = 1.0, TREB_BANDS),
        AnalysisFrame {
            // A transient is broadband, so the onset stimulus lights the whole
            // array rather than a slice.
            onset: 1.0,
            beat: true,
            spectrum: [1.0; SPECTRUM_BINS],
            ..Default::default()
        },
    ]
}

fn loud_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        // "Every band up" includes the log-band array itself — see
        // [`BASS_BANDS`].
        spectrum: [1.0; SPECTRUM_BINS],
        ..Default::default()
    }
}

fn corner(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        255,
    ]
}

fn print_text_report(source: &str, reports: &[FamilyReport]) {
    println!("visual-QA report [{source}]");
    for fam in reports {
        println!(
            "\n=== {} ({} presets) ===",
            fam.system.as_str(),
            fam.presets.len()
        );
        println!(
            "  {:<14} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
            "preset", "bass", "mid", "treb", "onset", "anim", "cover"
        );
        for p in &fam.presets {
            println!(
                "  {:<14.14} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3}",
                p.name,
                p.reactivity[0],
                p.reactivity[1],
                p.reactivity[2],
                p.reactivity[3],
                p.animation,
                p.coverage,
            );
        }
        if fam.near_dups.is_empty() {
            println!("  near-duplicate geometry: none below shape {NEAR_DUP_STRUCT}");
        } else {
            for (a, b) in &fam.near_dups {
                println!("  NEAR-DUP: {a} ~ {b}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Hand-rolled JSON (fixed numeric schema, no serde)
// ---------------------------------------------------------------------------

fn render_json(source: &str, reports: &[FamilyReport]) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&format!("\"source\":{},", json_string(source)));
    out.push_str("\"families\":{");
    for (fi, fam) in reports.iter().enumerate() {
        if fi > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}:{{", json_string(fam.system.as_str())));
        // presets
        out.push_str("\"presets\":{");
        for (pi, p) in fam.presets.iter().enumerate() {
            if pi > 0 {
                out.push(',');
            }
            out.push_str(&format!("{}:{{", json_string(&p.name)));
            out.push_str(&format!(
                "\"reactivity\":{{\"bass\":{},\"mid\":{},\"treb\":{},\"onset\":{}}},",
                num(p.reactivity[0]),
                num(p.reactivity[1]),
                num(p.reactivity[2]),
                num(p.reactivity[3]),
            ));
            out.push_str(&format!("\"animation\":{},", num(p.animation)));
            out.push_str(&format!("\"coverage\":{}", num(p.coverage)));
            out.push('}');
        }
        out.push_str("},");
        // distinctness
        out.push_str("\"distinctness\":{");
        out.push_str(&format!("\"pixel\":{},", json_matrix(&fam.pixel)));
        out.push_str(&format!("\"shape\":{},", json_matrix(&fam.shape)));
        out.push_str("\"near_duplicates\":[");
        for (di, (a, b)) in fam.near_dups.iter().enumerate() {
            if di > 0 {
                out.push(',');
            }
            out.push_str(&format!("[{},{}]", json_string(a), json_string(b)));
        }
        out.push(']');
        out.push('}');
        out.push('}');
    }
    out.push_str("}}");
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Label rasterizer over the shared 5x7 bitmap font
// ---------------------------------------------------------------------------

/// Draw `text` (uppercased) at `(x, y)` on `canvas`, each glyph scaled by
/// `scale`. Unknown characters render blank. Pixels outside the canvas are
/// clipped. The glyph table is [`standalone::shot::glyph`]; only the blit is
/// here, because it needs the dev-only `image` canvas.
fn draw_label(
    canvas: &mut image::RgbaImage,
    x: u32,
    y: u32,
    text: &str,
    color: [u8; 4],
    scale: u32,
) {
    let mut cx = x;
    for ch in text.chars() {
        let glyph = glyph_for(ch.to_ascii_uppercase());
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..GLYPH_COLS {
                if bits & (1 << (GLYPH_COLS - 1 - col)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = cx + col * scale + dx;
                            let py = y + row as u32 * scale + dy;
                            if px < canvas.width() && py < canvas.height() {
                                canvas.put_pixel(px, py, image::Rgba(color));
                            }
                        }
                    }
                }
            }
        }
        cx += GLYPH_ADVANCE * scale;
    }
}
