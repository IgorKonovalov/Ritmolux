//! `milkconv` — convert a `.milk` preset into an LMV bundle.
//!
//! ```text
//! milkconv <input.milk> [--out <file.toml>]
//! ```
//!
//! Writes the bundle to `--out`, or to stdout when no `--out` is given. Every
//! finding the converter had is printed to **stderr** and repeated in the
//! bundle's header, so a shell pipeline gets a clean preset on stdout and a
//! human gets the warnings either way.
//!
//! Exits non-zero with a message on a bad argument, an unreadable file, or a
//! preset that does not compile — the same contract the `shot` CLI keeps.

use std::io::Write as _;
use std::path::PathBuf;

use milkconv::{convert, milk};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("milkconv: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" | "-o" => {
                out = Some(PathBuf::from(
                    args.next().ok_or("--out needs a path".to_string())?,
                ));
            }
            "--help" | "-h" => {
                println!("usage: milkconv <input.milk> [--out <file.toml>]");
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
    let input = input.ok_or("usage: milkconv <input.milk> [--out <file.toml>]".to_string())?;

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
