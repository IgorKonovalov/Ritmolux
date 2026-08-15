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
//!   --set k=v,...            constant stimulus — a *held* value, not a
//!                            transient. bass/mid/treb/onset (normalized 0..1),
//!                            their *_raw twins, beat/bar/tempo/novelty, and the
//!                            clock beat_index/time_since_beat/beat_in_bar/
//!                            bar_index/bar_phase
//!   --frames <N>             frames to advance before capture (default 120)
//!   --size <WxH>             render size (default 1280x720)
//!   --out <path>             output PNG (single shot) or dir/file (--all)
//!   --all                    contact sheet of every preset (needs --out)
//!   --report [family=<sys>]  per-family reactivity / animation / distinctness
//!            [--json]        emit JSON instead of a text table
//!   --signal <kind:param>    synth audio filmstrip (click:120, dynamic:110, ...)
//!   --audio <clip.wav>       filmstrip from a 16-bit PCM WAV
//!   --strip <N>              frames tiled along the audio (default 8)
//!   --at <hop>,...           explicit filmstrip hops, beating --strip's even
//!                            spacing — the only way to aim a capture at a
//!                            transient (the level table names the onset peak)
//!   --frame-at <hop>         ONE frame at that hop, written at the full --size
//!                            with no tile scaling and no border. Needs --signal
//!                            or --audio; mutually exclusive with --at
//!   --tier floor|rich        quality tier to capture at (default floor).
//!                            A Rich capture is an instrument, never a baseline
//!   --horizon <minutes>      long-run drift check: render N SIMULATED minutes
//!                            at capture cadence and print one statistics row
//!                            per interval. Slow by construction; never a gate
//!   --interval <secs>        simulated seconds between horizon rows (default 30)
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
use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::{Preset, SystemKind, default_presets, load_dir};
use lmv_core::render::{CaptureImage, Tier};
use standalone::shot::args::{
    BandLevels, apply_set, band_levels, parse_hops, parse_size, synth_signal,
};
use standalone::shot::film::{StripLayout, check_hops, filmstrip_indices, filmstrip_layout};
use standalone::shot::glyph::{GLYPH_ADVANCE, GLYPH_COLS, glyph_for};
use standalone::shot::horizon;
use standalone::shot::renderer;
use standalone::shot::report;
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
    /// `--horizon <minutes>`: the long-run drift check (Plan 0085 Phase 1).
    Horizon,
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
    /// `--at <hop>,...`: explicit filmstrip hop indices, overriding `--strip`'s
    /// even spacing. The only way to aim a capture at a *transient* — see
    /// [`parse_hops`].
    at: Option<Vec<u32>>,
    /// `--frame-at <hop>`: capture that one hop and write it at the full
    /// `--size`, unscaled and unbordered.
    ///
    /// The gap this closes (Plan 0088 Phase 1). `--frames N` reaches full size
    /// but runs under **silence**, so a band-driven preset photographs at its
    /// resting state; `--at <hop>` runs the real analyzer over real dynamics but
    /// tiles into a filmstrip, which scales every frame to [`STRIP_H`] and draws
    /// a gutter around it. Neither produces a documentation image, and
    /// [ADR-0100] needs one. The hop is the same index `--at` takes and the
    /// level table reports.
    ///
    /// [`STRIP_H`]: standalone::shot::film::STRIP_H
    /// [ADR-0100]: ../../docs/adrs/0100-documentation-images-are-committed-headless-renders.md
    frame_at: Option<u32>,
    /// `--tier floor|rich`: the quality tier to capture at. **Floor by default**
    /// — a capture is a pure function of its inputs (NFR §6) and every golden
    /// baseline is blessed at the floor, so raising it is an explicit act. There
    /// is deliberately no `LMV_TIER` read here: an ambient environment variable
    /// silently changing what a capture renders is the reproducibility hazard the
    /// pin exists to prevent (ADR-0045).
    tier: Tier,
    /// `--horizon <minutes>`: simulated minutes to render in the long-run drift
    /// mode (Plan 0085 Phase 1). `None` leaves the mode off, which is what every
    /// other invocation of this CLI wants — a horizon is minutes of wall clock.
    horizon: Option<f32>,
    /// `--interval <secs>`: simulated seconds between horizon rows.
    interval: f32,
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
            at: None,
            frame_at: None,
            tier: Tier::Floor,
            horizon: None,
            // Thirty seconds: fine enough that a one-minute smoke run has rows to
            // compare, coarse enough that a ten-minute horizon prints twenty of
            // them rather than a page nobody reads.
            interval: 30.0,
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
            "--tier" => {
                let value = next_value(&mut it, "--tier")?;
                args.tier = Tier::from_name(&value)
                    .ok_or_else(|| format!("--tier `{value}`: expected `floor` or `rich`"))?;
            }
            "--signal" => args.signal = Some(next_value(&mut it, "--signal")?),
            "--audio" => args.audio = Some(PathBuf::from(next_value(&mut it, "--audio")?)),
            "--strip" => {
                args.strip = next_value(&mut it, "--strip")?
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or("--strip expects a positive integer")?;
            }
            "--horizon" => {
                let value = next_value(&mut it, "--horizon")?;
                args.horizon = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| format!("--horizon expects minutes, got `{value}`"))?,
                );
                args.mode = Mode::Horizon;
            }
            "--interval" => {
                let value = next_value(&mut it, "--interval")?;
                args.interval = value
                    .parse::<f32>()
                    .map_err(|_| format!("--interval expects seconds, got `{value}`"))?;
            }
            "--at" => args.at = Some(parse_hops(&next_value(&mut it, "--at")?)?),
            "--frame-at" => {
                let value = next_value(&mut it, "--frame-at")?;
                args.frame_at = Some(value.parse::<u32>().map_err(|_| {
                    format!("--frame-at expects a single hop index, got `{value}`")
                })?);
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
    if args.frame_at.is_some() {
        // Both flags name a hop to capture; accepting them together would mean
        // silently picking one, and there is no reading under which that is what
        // the caller meant.
        if args.at.is_some() {
            return Err(
                "--frame-at and --at both choose which hop to capture: pass one \
                 (--frame-at for a single full-size frame, --at for a filmstrip)"
                    .to_string(),
            );
        }
        if args.signal.is_none() && args.audio.is_none() {
            return Err(
                "--frame-at needs audio to advance through: add --signal <kind:param> \
                 (e.g. dynamic:110) or --audio <clip.wav>"
                    .to_string(),
            );
        }
    }
    // A horizon holds one stimulus for its whole run, which is what makes it
    // deterministic and what makes a row at minute nine comparable with a row at
    // minute one. A clip cannot serve that — `--signal` synthesizes four seconds
    // — so accepting both would mean silently ignoring one of them.
    if args.horizon.is_some() && (args.signal.is_some() || args.audio.is_some()) {
        return Err(
            "--horizon holds a single stimulus for the whole run and cannot be driven \
             by a clip: drop --signal/--audio and set the level with --set"
                .to_string(),
        );
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
         --set k=v,...              bass,mid,treb,onset (normalized 0..1) and\n\
         their bass_raw,mid_raw,treb_raw,onset_raw twins,\n\
         bar,beat,tempo,novelty,beat_index,time_since_beat,\n\
         beat_in_bar,bar_index,bar_phase\n\
         (each HELD for every captured frame - see docs/capturing.md)\n\
         --frames <N>               frames before capture (default 120)\n\
         --size <WxH>               render size (default 1280x720)\n\
         --out <path>               PNG path (shot) or dir/file (--all)\n\
         --all                      contact sheet of every preset (needs --out)\n\
         --report [family=<sys>]    metrics table (fragment_field | swarm)\n\
         --json                     emit the report as JSON\n\
         --signal <kind:param>      synth audio filmstrip: click:120 bass:60\n\
                                    treble:10000 noise:7 chord dynamic:110\n\
                                    (needs --out)\n\
         --audio <clip.wav>         filmstrip from a 16-bit PCM WAV (needs --out)\n\
         --strip <N>                frames tiled along the audio (default 8)\n\
         --at <hop>,...             explicit filmstrip hops (beats --strip) -\n\
                                    aim at the transient the level table names\n\
         --frame-at <hop>           ONE frame at that hop, at the full --size:\n\
                                    no tile scaling, no border. Needs --signal\n\
                                    or --audio; not combinable with --at\n\
         --tier floor|rich          quality tier to capture at (default floor)\n\
                                    rich is an INSTRUMENT, never a baseline\n\
         --horizon <minutes>        long-run drift check: N SIMULATED minutes at\n\
                                    capture cadence, one row per interval.\n\
                                    Minutes of wall clock; never a gate\n\
         --interval <secs>          simulated seconds between horizon rows\n\
                                    (default 30)"
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
        // The report machinery lives in the library (Plan 0061 Phase 4), so it
        // takes the three `Args` fields it reads rather than `Args` itself.
        Mode::Report => report::run(presets, &source, args.tier, args.family, args.json),
        Mode::Horizon => horizon::run(
            presets,
            &source,
            // Same reason as `report` above: the library owns the request's
            // shape, the example owns how a command line spells it.
            &horizon::HorizonRequest {
                preset: args.preset.clone(),
                stimulus: args.stimulus,
                minutes: args.horizon.unwrap_or_default(),
                interval_secs: args.interval,
                width: args.width,
                height: args.height,
                tier: args.tier,
                json: args.json,
            },
        ),
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
    let mut r = renderer(args.width, args.height, presets, args.tier)?;
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
    let mut r = renderer(args.width, args.height, presets, args.tier)?;

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

    let mut r = renderer(args.width, args.height, presets, args.tier)?;
    // An explicitly named hop beats the even spacing, whichever flag named it.
    // Both are range-checked against this clip — see [`check_hops`].
    let at = match (args.frame_at, &args.at) {
        (Some(hop), _) => {
            check_hops(&[hop], pcm.len(), format, "--frame-at")?;
            vec![hop]
        }
        (None, Some(hops)) => {
            check_hops(hops, pcm.len(), format, "--at")?;
            hops.clone()
        }
        (None, None) => filmstrip_indices(pcm.len(), format, args.strip)?,
    };
    let frames = r
        .capture_audio(&name, &pcm, format, &at)
        .map_err(|e| format!("capture audio: {e}"))?;

    // Same clip, same analysis, same hop arithmetic as the strip — the only
    // difference is that one frame goes to disk at its captured size instead of
    // being scaled into a tile.
    let summary = match args.frame_at {
        Some(hop) => {
            let frame = frames
                .first()
                .ok_or_else(|| format!("--frame-at {hop}: no frame captured"))?;
            save_png(frame, &out)?;
            format!(
                "{}x{}, preset {name}, hop {hop}, {label}",
                frame.width, frame.height
            )
        }
        None => {
            let strip = tile_filmstrip(&frames)?;
            save_image(&strip, &out)?;
            format!("{} frames, preset {name}, {label}", frames.len())
        }
    };
    // Printed unconditionally rather than behind a flag: an author who does not
    // already know that `--set` magnitudes are unlike real levels is exactly the
    // one who will never pass the flag.
    print_band_levels(&band_levels(&pcm, format)?);
    println!("wrote {} ({summary}) [{source}]", out.display());
    Ok(())
}

/// Report what the analyzer derived from this clip, so "what does real material
/// actually produce" is answered with numbers instead of a guess.
///
/// `onset` sits in the table beside the three bands (Plan 0057 Phase 1) because
/// every shipped attractor gates its `reseed` on it, and whether a given
/// `--signal` kind ever crosses such a gate was previously unanswerable from a
/// capture. The peak hop is printed with it so `--strip` can be aimed at the
/// frame the gate fires on.
fn print_band_levels(levels: &BandLevels) {
    println!(
        "audio levels over {} analysis hops (past warm-up) — calibrate gains against these, \
         not against --set magnitudes:",
        levels.hops
    );
    println!("  {:<6} {:>8} {:>8} {:>8}", "signal", "min", "mean", "max");
    for (name, band) in [
        ("bass", levels.bass),
        ("mid", levels.mid),
        ("treb", levels.treb),
        ("onset", levels.onset),
    ] {
        println!(
            "  {name:<6} {:>8.3} {:>8.3} {:>8.3}",
            band.min, band.mean, band.max
        );
    }
    println!(
        "  onset peaks at {:.3} on hop {} — the shipped attractor reseed gates run \
         0.50 to 0.75",
        levels.onset.max, levels.onset_peak_hop
    );
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
