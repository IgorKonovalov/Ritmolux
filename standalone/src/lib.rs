//! Host-side helpers shared by the `lmv` binary and the `shot` example: the
//! per-OS path resolution below, plus [`shot`]'s pure CLI helpers (which live
//! here so `cargo test` actually runs their tests — an `examples/` target's
//! `#[test]` does not).
//!
//! ## Preset-directory resolution
//!
//! The per-OS preset directory used to be hand-copied into `src/main.rs` and
//! `examples/shot.rs`; the two copies could drift, which silently breaks the
//! one invariant the `LMV_PRESET_DIR` override rests on — the app and `shot`
//! must resolve the *same* directory (ADR-0014). This module is that single
//! source. It is host-only by design: `%APPDATA%` / `HOME` / `XDG` conventions
//! are a shell concern and never leak into the source-agnostic core.
//!
//! Nothing here prints or panics — callers decide how to report an unresolved
//! root, so the headless `shot` stays quiet where the app logs.
//!
//! **This covers the Rust side only.** The foobar2000 plugin resolves the same
//! `%APPDATA%` directory independently in C++ (`plugin-foobar/foo_lmv.cpp`,
//! `resolve_preset_dir_utf8`) because it is compiled separately, and it does
//! **not** honor [`PRESET_DIR_ENV`]. A change to [`APP_DIR_NAME`] or the layout
//! below has to be mirrored there by hand.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub mod shot;

/// Per-user application directory name, used under the OS data root for the
/// shared preset directory, the diagnostics log, and `config.toml`.
pub const APP_DIR_NAME: &str = "light-music-visualizer";

/// Environment variable naming a preset directory that overrides the per-user
/// default (ADR-0014). Set it to the repo's `presets/` for the edit-live loop,
/// or to any folder to run a custom preset library.
pub const PRESET_DIR_ENV: &str = "LMV_PRESET_DIR";

/// Where the preset directory came from, so the app knows whether to seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetDir {
    /// `LMV_PRESET_DIR` pointed here — user-owned, so we never seed into it.
    Override(PathBuf),
    /// The per-OS `%APPDATA%` / `HOME` / `XDG` default — seeded write-if-absent
    /// on first run.
    Default(PathBuf),
    /// No OS data root could be resolved and no override was given; the caller
    /// keeps the embedded defaults (degrade, never crash — NFR 10).
    Unresolved,
}

impl PresetDir {
    /// The resolved directory, or `None` when nothing could be resolved.
    pub fn path(&self) -> Option<&Path> {
        match self {
            PresetDir::Override(dir) | PresetDir::Default(dir) => Some(dir),
            PresetDir::Unresolved => None,
        }
    }

    /// True when `LMV_PRESET_DIR` supplied the directory.
    pub fn is_override(&self) -> bool {
        matches!(self, PresetDir::Override(_))
    }
}

/// Resolve the preset directory: `LMV_PRESET_DIR` wins when set to a non-empty
/// path, otherwise the per-OS default under [`preset_data_root`], otherwise
/// [`PresetDir::Unresolved`].
pub fn resolve_preset_dir() -> PresetDir {
    resolve_preset_dir_from(std::env::var_os(PRESET_DIR_ENV), preset_data_root())
}

/// The resolution rule as a pure function of its two inputs, so it is testable
/// without touching process-global environment state.
fn resolve_preset_dir_from(env_override: Option<OsString>, root: Option<PathBuf>) -> PresetDir {
    if let Some(dir) = env_override.filter(|v| !v.is_empty()) {
        return PresetDir::Override(PathBuf::from(dir));
    }
    match root {
        Some(root) => PresetDir::Default(root.join(APP_DIR_NAME).join("presets")),
        None => PresetDir::Unresolved,
    }
}

/// The OS data root under which the per-user app directory lives, hand-rolled
/// per-OS so we add no runtime dependency (NFR 4). Windows: `%APPDATA%`.
/// macOS: `~/Library/Application Support`. Other: `$XDG_DATA_HOME` (or
/// `~/.local/share`). `None` when the environment names none of them.
///
/// Unaffected by [`PRESET_DIR_ENV`] — the diagnostics log and `config.toml`
/// stay under the per-user root even when the presets come from elsewhere.
#[cfg(windows)]
pub fn preset_data_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
pub fn preset_data_root() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        })
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn preset_data_root() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".local").join("share"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins_and_is_tagged_as_such() {
        let root = Some(PathBuf::from("/data"));
        assert_eq!(
            resolve_preset_dir_from(Some(OsString::from("/repo/presets")), root.clone()),
            PresetDir::Override(PathBuf::from("/repo/presets"))
        );
        // An override survives having no OS data root at all.
        assert_eq!(
            resolve_preset_dir_from(Some(OsString::from("/repo/presets")), None),
            PresetDir::Override(PathBuf::from("/repo/presets"))
        );
        // Empty is treated as unset, not as "the current directory".
        assert_eq!(
            resolve_preset_dir_from(Some(OsString::new()), root),
            PresetDir::Default(PathBuf::from("/data").join(APP_DIR_NAME).join("presets"))
        );
    }

    #[test]
    fn without_an_override_the_per_os_default_applies() {
        assert_eq!(
            resolve_preset_dir_from(None, Some(PathBuf::from("/data"))),
            PresetDir::Default(PathBuf::from("/data").join(APP_DIR_NAME).join("presets"))
        );
        assert_eq!(resolve_preset_dir_from(None, None), PresetDir::Unresolved);
    }

    /// Both halves of the env-var contract live in **one** test on purpose:
    /// `set_var` mutates process-global state, so splitting them would let a
    /// threaded `cargo test` interleave the set with the unset assertion. This
    /// is the only test in the crate that touches the environment.
    #[test]
    fn resolve_preset_dir_reads_the_environment() {
        let wanted = PathBuf::from("presets");
        // SAFETY: single-threaded within this test, and no other test in the
        // crate reads or writes an environment variable.
        unsafe { std::env::set_var(PRESET_DIR_ENV, &wanted) };
        let resolved = resolve_preset_dir();
        // SAFETY: as above — restore the ambient environment before asserting,
        // so a failed assertion can't leak the override into other tests.
        unsafe { std::env::remove_var(PRESET_DIR_ENV) };
        assert_eq!(resolved, PresetDir::Override(wanted));

        let expected = match preset_data_root() {
            Some(root) => PresetDir::Default(root.join(APP_DIR_NAME).join("presets")),
            None => PresetDir::Unresolved,
        };
        assert_eq!(resolve_preset_dir(), expected);
        assert!(!resolve_preset_dir().is_override());
    }
}
