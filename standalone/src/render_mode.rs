//! The player's two offline answer-and-exit modes over a WAV clip (ADR-0262).
//!
//! **`--render <wav> --preset <name>`** writes the preset's video over the clip
//! to standard output as a Y4M stream and exits. It is `shot --render`'s walk,
//! called rather than copied: [`render::run`] owns the two clocks, the wire
//! format and the offline renderer, so a file the player renders is byte for
//! byte the file `shot` renders from the same preset, clip, rate, size and tier.
//!
//! **`--bars <wav> --out <path>`** writes the bar grid that render would draw —
//! [`render::bar_grid`]'s JSON, the file `shot --render --bar-grid` writes — and
//! exits. It walks the analyzer only: nothing on its path builds a renderer or
//! asks for an adapter, which `the_bars_mode_holds_no_gpu_type` holds by
//! reading this file.
//!
//! Both are dispatched from `main` before the launch path, the way `--thumb` is:
//! neither opens a window, binds a socket, starts a capture client or seeds the
//! preset directory. Their caller is a program — the studio — so each refuses
//! any flag it does not read rather than ignoring it.
//!
//! **Exit codes follow the launch path's split**: 2 for an argument list wrong
//! in shape, including a preset no library holds, and 1 for a recognised request
//! whose effect failed — a clip that is not 16-bit PCM, a renderer that cannot
//! start, a file that cannot be written. Every refusal is one line on standard
//! error. Standard output carries the frame stream and nothing else.

use std::path::{Path, PathBuf};

use rlx_core::audio::AudioFormat;
use rlx_core::render::Tier;
use standalone::shot;
use standalone::shot::render::{self, Fps};

use crate::cli::{flag_name, flag_value};

/// The frame size a `--render` with no `--size` draws at: `shot`'s own default,
/// so the two spell the same render the same way.
pub(crate) const DEFAULT_SIZE: (u32, u32) = (1280, 720);

/// The tier a `--render` with no `--tier` draws at: `shot`'s floor, because a
/// render is a pure function of its inputs and the engine's own pick depends on
/// the machine (ADR-0045).
pub(crate) const DEFAULT_TIER: Tier = Tier::Floor;

/// The flags `--render` reads. Anything else on its command line is refused.
const RENDER_FLAGS: &[&str] = &["--render", "--preset", "--fps", "--size", "--tier"];

/// The flags `--bars` reads. Anything else on its command line is refused.
const BARS_FLAGS: &[&str] = &["--bars", "--fps", "--out"];

/// One parsed request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Request {
    Render {
        clip: PathBuf,
        preset: String,
        fps: Fps,
        width: u32,
        height: u32,
        tier: Tier,
    },
    Bars {
        clip: PathBuf,
        fps: Fps,
        out: PathBuf,
    },
}

/// The mode's exit code, or `None` when this run is neither mode and the
/// ordinary launch path should proceed.
pub(crate) fn mode() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(None) => None,
        Ok(Some(Request::Render {
            clip,
            preset,
            fps,
            width,
            height,
            tier,
        })) => Some(render_clip(&clip, preset, fps, (width, height), tier)),
        Ok(Some(Request::Bars { clip, fps, out })) => Some(bars(&clip, fps, &out)),
        Err(message) => {
            eprintln!("{message}");
            Some(2)
        }
    }
}

/// `message` with `flag` in front of it, unless it already names the flag.
fn named(flag: &str, message: String) -> String {
    if message.starts_with(flag) {
        message
    } else {
        format!("{flag}: {message}")
    }
}

/// Judge `args` (without the program name): `Ok(None)` when neither mode was
/// asked for, the request when one was, and the one-line reason otherwise.
///
/// `--help` anywhere yields `None`, so the roster is printed rather than a
/// refusal: someone asking what the flags are is the one caller to answer.
pub(crate) fn parse(args: &[String]) -> Result<Option<Request>, String> {
    let asks = |mode: &str| args.iter().any(|arg| flag_name(arg) == Some(mode));
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(None);
    }
    let (mode, allowed) = match (asks("--render"), asks("--bars")) {
        (false, false) => return Ok(None),
        (true, true) => {
            return Err("--render and --bars are two modes: pass one".to_owned());
        }
        (true, false) => ("--render", RENDER_FLAGS),
        (false, true) => ("--bars", BARS_FLAGS),
    };

    let mut values: Vec<(&str, String)> = Vec::new();
    let mut it = args.iter().cloned();
    while let Some(arg) = it.next() {
        let Some(name) = flag_name(&arg) else {
            return Err(format!("`{arg}`: {mode} takes no positional argument"));
        };
        let Some(&name) = allowed.iter().find(|known| **known == name) else {
            return Err(format!(
                "`{name}` is not read by {mode}, which takes {}",
                allowed.join(", ")
            ));
        };
        let value = flag_value(&arg, name, &mut it)
            .unwrap_or_else(|| Err(format!("{name}: expected a value")))?;
        if value.trim().is_empty() || flag_name(&value).is_some() {
            return Err(format!("{name}: expected a value"));
        }
        if values.iter().any(|(seen, _)| *seen == name) {
            return Err(format!("{name} is given twice"));
        }
        values.push((name, value));
    }
    let value = |name: &str| {
        values
            .iter()
            .find(|(seen, _)| *seen == name)
            .map(|(_, v)| v.clone())
    };

    let clip = PathBuf::from(value(mode).unwrap_or_default());
    let fps = match value("--fps") {
        Some(spec) => render::parse_fps(&spec).map_err(|e| named("--fps", e))?,
        None => render::DEFAULT_FPS,
    };
    if mode == "--bars" {
        let out = value("--out").ok_or("--bars needs --out <path> to write the grid to")?;
        return Ok(Some(Request::Bars {
            clip,
            fps,
            out: PathBuf::from(out),
        }));
    }

    let preset = value("--preset").ok_or("--render needs --preset <name>")?;
    let (width, height) = match value("--size") {
        Some(spec) => shot::args::parse_size(&spec).map_err(|e| named("--size", e))?,
        None => DEFAULT_SIZE,
    };
    let tier = match value("--tier") {
        Some(name) => Tier::from_name(&name)
            .ok_or_else(|| format!("--tier `{name}`: expected `floor` or `rich`"))?,
        None => DEFAULT_TIER,
    };
    Ok(Some(Request::Render {
        clip,
        preset,
        fps,
        width,
        height,
        tier,
    }))
}

/// Read `path` as a 16-bit PCM WAV, or the one-line reason it is not one.
fn read_clip(flag: &str, path: &Path) -> Result<(Vec<f32>, AudioFormat), String> {
    let label = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| format!("{flag} {label}: {e}"))?;
    shot::wav::parse_wav_16bit(&bytes, &label).map_err(|e| format!("{flag} {label}: {e}"))
}

/// `--render`: the preset's video over `clip`, as Y4M on standard output.
///
/// The preset is checked against the library **before** the clip is read and a
/// renderer is built, because a name no library holds is the spelling of the
/// request rather than its effect.
fn render_clip(clip: &Path, preset: String, fps: Fps, size: (u32, u32), tier: Tier) -> i32 {
    let presets = crate::thumbs::library();
    if !presets.iter().any(|p| p.name == preset) {
        eprintln!("--render: no preset named `{preset}` in this library (see --list-presets)");
        return 2;
    }
    let (pcm, format) = match read_clip("--render", clip) {
        Ok(clip) => clip,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    let request = render::RenderRequest {
        preset: Some(preset),
        fps,
        width: size.0,
        height: size.1,
        tier,
        encoder: None,
        bar_grid: None,
    };
    match render::run(
        presets,
        "player library",
        &request,
        &pcm,
        format,
        &clip.display().to_string(),
    ) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{}", named("--render", message));
            1
        }
    }
}

/// `--bars`: the bar grid a `--render` of `clip` at `fps` draws, written to
/// `out`. The analyzer walk only — see the module docs.
fn bars(clip: &Path, fps: Fps, out: &Path) -> i32 {
    let (pcm, format) = match read_clip("--bars", clip) {
        Ok(clip) => clip,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    let grid = match render::bar_grid(&pcm, format, fps) {
        Ok(grid) => grid,
        Err(message) => {
            eprintln!("{}", named("--bars", message));
            return 1;
        }
    };
    if let Err(err) = std::fs::write(out, grid.to_json()) {
        eprintln!("--out {}: {err}", out.display());
        return 1;
    }
    eprintln!("{} [{}]", grid.summary(), out.display());
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_owned()).collect()
    }

    fn refused(args: &[&str]) -> String {
        parse(&argv(args)).expect_err("the argument list was accepted")
    }

    /// Neither mode on the line is the ordinary launch, and so is `--help`.
    #[test]
    fn a_line_without_either_mode_is_left_to_the_launch_path() {
        assert_eq!(
            parse(&argv(&["--preset", "Gyre", "--tier", "rich"])),
            Ok(None)
        );
        assert_eq!(parse(&argv(&["--render", "a.wav", "--help"])), Ok(None));
    }

    /// Both spellings of every flag, the defaults `shot` has, and the request
    /// each mode carries.
    #[test]
    fn each_mode_parses_its_flags_and_takes_shots_defaults() {
        assert_eq!(
            parse(&argv(&["--render", "a.wav", "--preset", "Gyre"])),
            Ok(Some(Request::Render {
                clip: PathBuf::from("a.wav"),
                preset: "Gyre".to_owned(),
                fps: render::DEFAULT_FPS,
                width: 1280,
                height: 720,
                tier: Tier::Floor,
            }))
        );
        assert_eq!(
            parse(&argv(&[
                "--render=a.wav",
                "--preset=Lace Grid",
                "--fps=30000/1001",
                "--size",
                "1920x1080",
                "--tier",
                "rich",
            ])),
            Ok(Some(Request::Render {
                clip: PathBuf::from("a.wav"),
                preset: "Lace Grid".to_owned(),
                fps: Fps {
                    num: 30000,
                    den: 1001
                },
                width: 1920,
                height: 1080,
                tier: Tier::Rich,
            }))
        );
        assert_eq!(
            parse(&argv(&[
                "--bars", "a.wav", "--fps", "30", "--out", "g.json"
            ])),
            Ok(Some(Request::Bars {
                clip: PathBuf::from("a.wav"),
                fps: Fps { num: 30, den: 1 },
                out: PathBuf::from("g.json"),
            }))
        );
    }

    /// Every refusal names the flag that is wrong, on one line.
    #[test]
    fn a_wrong_shape_is_refused_naming_the_flag() {
        for (args, needle) in [
            (&["--render", "a.wav"][..], "--preset"),
            (&["--bars", "a.wav"], "--out"),
            (&["--render", "a.wav", "--bars", "b.wav"], "two modes"),
            (
                &["--render", "a.wav", "--preset", "G", "--stream"],
                "--stream",
            ),
            (
                &["--bars", "a.wav", "--out", "g", "--size", "8x8"],
                "--size",
            ),
            (
                &["--render", "a.wav", "--preset", "G", "--fps", "29.97"],
                "--fps",
            ),
            (
                &["--render", "a.wav", "--preset", "G", "--size", "8"],
                "--size",
            ),
            (
                &["--render", "a.wav", "--preset", "G", "--tier", "max"],
                "--tier",
            ),
            (&["--render", "--preset", "G"], "--render"),
            (
                &["--render", "a.wav", "--preset", "G", "--preset", "H"],
                "twice",
            ),
            (&["--render", "a.wav", "stray"], "stray"),
        ] {
            let message = refused(args);
            assert!(message.contains(needle), "{args:?}: {message}");
            assert_eq!(message.lines().count(), 1, "{args:?}: {message}");
        }
    }

    /// The body of the free function `name` in this file's source, up to the
    /// next item at column 0.
    fn body_of<'a>(source: &'a str, name: &str) -> &'a str {
        let start = source
            .find(&format!("\nfn {name}("))
            .unwrap_or_else(|| panic!("no `fn {name}` in render_mode.rs"));
        let rest = &source[start + 1..];
        let end = rest.find("\n}\n").map_or(rest.len(), |at| at + 3);
        &rest[..end]
    }

    /// **`--bars` holds no GPU type.** A structural property, so it is read off
    /// the source: the mode's body and the clip reader it calls name no renderer,
    /// no adapter, no tier and nothing of `wgpu`, while the render arm — the
    /// control that keeps this from passing on an empty body — names the
    /// renderer's entry point.
    #[test]
    fn the_bars_mode_holds_no_gpu_type() {
        let source = include_str!("render_mode.rs");
        let source = source.split("#[cfg(test)]").next().unwrap_or(source);
        for name in ["bars", "read_clip"] {
            let body = body_of(source, name);
            assert!(body.contains(&format!("fn {name}(")), "{body}");
            for gpu in [
                "Renderer",
                "renderer",
                "render::run",
                "wgpu",
                "Tier",
                "adapter",
            ] {
                assert!(
                    !body.contains(gpu),
                    "`fn {name}` names `{gpu}`, so --bars may build what it must not:\n{body}"
                );
            }
        }
        assert!(
            body_of(source, "render_clip").contains("render::run"),
            "the control arm no longer calls the renderer, so the scan above proves nothing"
        );
    }
}
