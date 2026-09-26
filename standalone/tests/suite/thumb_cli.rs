//! The `--thumb` mode, run the way the thumbnail pass runs it (ADR-0230).
//!
//! The mode's whole contract is observable only from outside the process: it
//! writes a file under the per-user data root and exits. `common::player` gives
//! each case a data root of its own, so a run here never touches the developer's
//! own cache, and `RLX_PRESET_DIR` points at a one-preset scratch library so the
//! stamp under test is a real file's.
//!
//! The rendering case needs a real adapter and **skips with a printed reason**
//! where there is none, on the rule `shot_cli` already follows: the skip is keyed
//! on the adapter error itself, so any other failure still fails.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Output;

/// Substring of the error a run prints when no GPU adapter can be acquired
/// (`RenderError::RequestAdapter`). Matching the adapter failure specifically
/// means an unrelated non-zero exit is never mistaken for a skip.
const NO_ADAPTER: &str = "no suitable GPU adapter";

/// The scratch library's single preset. Its own file rather than a shipped one:
/// the cases below are about the cache and the stamp, and a library of one is
/// what keeps the render they need to the one preset under test.
const PRESET_SRC: &str = r#"
system = "fragment_field"
name = "Thumb Probe"
[params]
zoom = "1.2"
"#;

const PRESET_NAME: &str = "Thumb Probe";

/// A fresh scratch directory inside the build tree, unique to this process and
/// this call — `CARGO_TARGET_TMPDIR` is per test binary and exists for this.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("thumb-cli")
        .join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch dir");
    dir
}

/// A one-preset library under `dir`, and the directory it lives in.
fn scratch_library(dir: &Path) -> PathBuf {
    let presets = dir.join("presets");
    std::fs::create_dir_all(&presets).expect("create the scratch library");
    std::fs::write(presets.join("probe.toml"), PRESET_SRC).expect("write the probe preset");
    presets
}

/// The player in `--thumb` mode, with `root` as its data root and `presets` as
/// its library.
fn thumb(root: &Path, presets: &Path, name: &str) -> Output {
    common::player_with_data_root(root)
        .env("RLX_PRESET_DIR", presets)
        .args(["--thumb", name])
        .output()
        .expect("spawning the ritmolux binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The cache directory a run whose data root env vars all name `root` writes
/// into — the per-OS arms of `standalone::preset_data_root`, plus the app dir.
fn cache_dir(root: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let root = root.join("Library").join("Application Support");
    #[cfg(not(target_os = "macos"))]
    let root = root.to_path_buf();
    root.join(standalone::APP_DIR_NAME).join("thumbnails")
}

/// Every file in the cache directory, or an empty list when it does not exist.
fn cached_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_dir(root)) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    files.sort();
    files
}

/// **The mode's whole contract, in one run and then a second.** One invocation
/// writes exactly one cached image; a second for the same unchanged preset
/// writes nothing and says why, which is what stops the pass from re-rendering
/// the library on every launch.
#[test]
fn one_invocation_writes_one_image_and_the_next_is_a_reported_no_op() {
    let dir = scratch("one-image");
    let presets = scratch_library(&dir);
    let root = dir.join("data");
    std::fs::create_dir_all(&root).expect("create the data root");

    let first = thumb(&root, &presets, PRESET_NAME);
    if !first.status.success() && stderr(&first).contains(NO_ADAPTER) {
        eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
        return;
    }
    assert!(
        first.status.success(),
        "the first --thumb run failed\nstdout: {}\nstderr: {}",
        stdout(&first),
        stderr(&first)
    );

    let files = cached_files(&root);
    assert_eq!(
        files.len(),
        1,
        "one --thumb run wrote {} file(s): {files:?}",
        files.len()
    );
    let image = &files[0];
    assert_eq!(
        image.extension().and_then(|e| e.to_str()),
        Some("rlxthumb"),
        "the run left a file that is not a cache entry: {}",
        image.display()
    );

    // The bytes are an entry of the size this build renders, read the way the
    // browser reads them: magic, version, then the two dimensions.
    let bytes = std::fs::read(image).expect("read the cache entry");
    assert_eq!(&bytes[..4], b"RLXT", "the entry has no magic");
    let at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    assert_eq!(
        at(4),
        1,
        "the entry declares a version this test cannot read"
    );
    assert_eq!((at(8), at(12)), (160, 90), "the still is not 160x90");

    let written = std::fs::metadata(image)
        .and_then(|m| m.modified())
        .expect("the entry has a modification time");

    // The second run: the preset has not changed, so nothing is rendered.
    let second = thumb(&root, &presets, PRESET_NAME);
    assert!(
        second.status.success(),
        "the second --thumb run failed\nstderr: {}",
        stderr(&second)
    );
    assert_eq!(
        cached_files(&root),
        files,
        "the no-op run changed what is in the cache"
    );
    assert_eq!(
        std::fs::metadata(image).and_then(|m| m.modified()).ok(),
        Some(written),
        "the no-op run rewrote the entry"
    );
    assert!(
        stderr(&second).contains("up to date"),
        "the no-op run did not say why it did nothing:\n{}",
        stderr(&second)
    );
}

/// **A name no library holds is refused, and the mode is not advertised.** Both
/// exit before a renderer exists, so they run everywhere.
#[test]
fn a_bad_invocation_is_refused_and_help_never_names_the_mode() {
    let dir = scratch("refused");
    let presets = scratch_library(&dir);
    let root = dir.join("data");
    std::fs::create_dir_all(&root).expect("create the data root");

    let out = thumb(&root, &presets, "No Such Preset");
    assert!(!out.status.success(), "an unknown preset name exited 0");
    assert!(
        stderr(&out).contains("No Such Preset"),
        "the refusal does not name what was asked for:\n{}",
        stderr(&out)
    );
    assert!(
        cached_files(&root).is_empty(),
        "a refused run wrote into the cache"
    );

    // The flag exists for the parent process, so the roster an operator reads
    // must not offer it.
    let help = common::player()
        .arg("--help")
        .output()
        .expect("spawning the ritmolux binary");
    assert!(help.status.success(), "`--help` did not exit 0");
    assert!(
        !stdout(&help).contains("--thumb"),
        "--help advertises the thumbnail mode:\n{}",
        stdout(&help)
    );
}
