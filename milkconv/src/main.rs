//! `milkconv` — convert a `.milk` preset into an LMV bundle.
//!
//! ```text
//! milkconv <input.milk> [--out <file.toml>]
//! milkconv --report <dir> [--render] [--json] [--out <file>]
//! ```
//!
//! Writes the bundle to `--out`, or to stdout when no `--out` is given. Every
//! finding the converter had is printed to **stderr** and repeated in the
//! bundle's header, so a shell pipeline gets a clean preset on stdout and a
//! human gets the warnings either way.
//!
//! `--report` is Plan 0100 Phase 5: run the converter over a whole corpus and
//! print what happened, ranked. **It asserts no threshold** (ADR-0071) and exits
//! zero however bad the numbers are — see `report.rs`.
//!
//! Exits non-zero with a message on a bad argument, an unreadable file, or a
//! preset that does not compile — the same contract the `shot` CLI keeps.

use std::io::Write as _;
use std::path::PathBuf;

use milkconv::{convert, milk, report};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("milkconv: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage:\n  \
     milkconv <input.milk> [--out <file.toml>]\n  \
     milkconv --report <dir> [--render] [--json] [--out <file>]\n\n\
     --report walks <dir> for .milk files and prints what became of each.\n\
     --render additionally loads every converted preset into a headless\n\
     renderer and counts how many put light on screen; it needs a GPU and it\n\
     is the slow half. The report asserts no threshold (ADR-0071).";

fn run() -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut report_dir: Option<PathBuf> = None;
    let mut do_render = false;
    let mut as_json = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                out = Some(PathBuf::from(
                    args.next().ok_or("--out needs a path".to_string())?,
                ));
            }
            "--report" => {
                report_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or("--report needs a directory".to_string())?,
                ));
            }
            "--render" => do_render = true,
            "--json" => as_json = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag '{other}'"));
            }
            other => {
                if input.is_some() {
                    return Err("only one input file at a time".into());
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    if let Some(dir) = report_dir {
        return run_report(&dir, do_render, as_json, out.as_deref());
    }
    if do_render || as_json {
        return Err("--render and --json belong to --report".into());
    }
    let input = input.ok_or(USAGE.to_string())?;

    // Read as bytes and convert lossily: `.milk` files are Windows-era text and
    // a few carry a Latin-1 byte in their title. Refusing one over its name would
    // lose a preset that converts fine.
    let bytes = std::fs::read(&input).map_err(|e| format!("read {}: {e}", input.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let file = milk::parse(&text).map_err(|e| format!("{}: {e}", input.display()))?;

    let name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let converted =
        convert::convert(&file, name).map_err(|e| format!("{}: {e}", input.display()))?;

    for warning in &converted.warnings {
        eprintln!("milkconv: {name}: {}", warning.message);
    }

    match out {
        Some(path) => {
            std::fs::write(&path, converted.toml)
                .map_err(|e| format!("write {}: {e}", path.display()))?;
            eprintln!("milkconv: wrote {}", path.display());
        }
        None => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle
                .write_all(converted.toml.as_bytes())
                .map_err(|e| format!("write stdout: {e}"))?;
        }
    }
    Ok(())
}

/// Plan 0100 Phase 5's `--report`.
///
/// **Exits zero however bad the numbers are.** The whole point of ADR-0071 is
/// that a coverage percentage is not a gate; a non-zero exit here would make it
/// one the first time somebody put it in a script. The only failures are a
/// missing directory and an unwritable `--out`.
fn run_report(
    dir: &std::path::Path,
    do_render: bool,
    as_json: bool,
    out: Option<&std::path::Path>,
) -> Result<(), String> {
    let paths = report::collect(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    if paths.is_empty() {
        return Err(format!("no .milk files under {}", dir.display()));
    }
    eprintln!("milkconv: converting {} files...", paths.len());
    let mut measured = report::run(&paths, |done, total| {
        if done > 0 {
            eprintln!("  convert {done}/{total}");
        }
    });

    let mut rendered = false;
    if do_render {
        eprintln!("milkconv: rendering probes (this is the slow half)...");
        match report::render_probe(&mut measured, |done, total| {
            if done > 0 {
                eprintln!("  render {done}/{total}");
            }
        }) {
            Ok(()) => rendered = true,
            // Not fatal, and deliberately: a box with no adapter still gets the
            // two counts that do not need one (ADR-0016's policy for captures).
            Err(e) => eprintln!("milkconv: skipping the render probe - {e}"),
        }
    }

    let text = if as_json {
        measured.to_json()
    } else {
        measured.render(
            &dir.display().to_string(),
            &format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            rendered,
        )
    };
    match out {
        Some(path) => {
            std::fs::write(path, &text).map_err(|e| format!("write {}: {e}", path.display()))?;
            eprintln!("milkconv: wrote {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}
