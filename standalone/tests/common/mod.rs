//! The one constructor for a spawned workspace binary in `standalone/tests/`.
//!
//! `standalone/tests/common/` is a directory, not a top-level `.rs`, so cargo
//! does not compile it as its own test binary; each test file pulls it in with
//! `mod common;`.
//!
//! **Every command handed out here has its own per-user data root.** The player
//! and `shot` resolve that root from `APPDATA` on Windows, `HOME` on macOS and
//! `XDG_DATA_HOME` (else `HOME`) elsewhere (`standalone::preset_data_root`).
//! Left inherited, a spawned run migrates the developer's real app directory,
//! appends to its `diagnostics.log`, binds whatever `[control]` port its
//! `config.toml` names and reads its preset directory as test input, none of
//! which a reader of the test can see. So all three variables are pointed at a
//! directory under `CARGO_TARGET_TMPDIR`, and the caller adds arguments, the
//! working directory and pipes.
//!
//! `core/tests/hygiene.rs` fails on a file under `standalone/tests/` other than
//! this one that names the player's `CARGO_BIN_EXE_` variable or `shot_bin`, so a
//! spawn written beside this module is refused rather than reviewed for.
//!
//! Not every file uses every helper, so the module allows dead code: `-D warnings`
//! would otherwise fail the build of a file that needs only one of them.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The player, `ritmolux`, with a fresh data root of its own.
pub fn player() -> Command {
    player_with_data_root(&scratch_data_root())
}

/// The player with its data root pointed at `root`.
///
/// For a test that reads back what the run wrote under the root, such as the
/// diagnostics log. An **empty** `root` is the unresolved case: every variable is
/// set and empty, which `preset_data_root` reads as unset, so the run keeps the
/// embedded set and says why.
pub fn player_with_data_root(root: &Path) -> Command {
    with_data_root(env!("CARGO_BIN_EXE_ritmolux"), root)
}

/// The `shot` example, with a fresh data root of its own.
pub fn shot() -> Command {
    with_data_root(shot_bin(), &scratch_data_root())
}

/// The `shot` example's path, for a test that hands it to `shot` as an argument
/// (a stand-in encoder). Never pass it to `Command::new`: [`shot`] is the spawn.
pub fn shot_executable() -> PathBuf {
    shot_bin()
}

/// A new, empty directory under `CARGO_TARGET_TMPDIR`, unique to this process
/// and this call.
///
/// Keyed by process id and a per-process counter: nextest runs every test in its
/// own process, and a test that spawns twice needs two roots.
pub fn scratch_data_root() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("data-root")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch data root");
    dir
}

fn with_data_root(program: impl AsRef<OsStr>, root: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env("APPDATA", root)
        .env("HOME", root)
        .env("XDG_DATA_HOME", root);
    command
}

/// Locate `<target>/<profile>/examples/shot[.exe]` by walking up from this test
/// binary. The test lives at `<target>/<profile>/deps/<binary>-<hash>`, so the
/// `examples/` sibling is one or two levels up. Searching the ancestors instead
/// of hardcoding the depth keeps this working under `CARGO_TARGET_DIR` and under
/// a `--target <triple>` layout.
///
/// `shot` is an example rather than a `[[bin]]`, so no `CARGO_BIN_EXE_shot`
/// names it: `image` is a dev-dependency to keep the PNG codec out of the
/// shipped binary, and a `[[bin]]` does not get dev-dependencies (ADR-0033
/// Alternative E).
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
