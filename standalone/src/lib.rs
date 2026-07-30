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

/// Environment variable pinning the quality tier — `floor` or `rich`
/// (Plan 0044 / ADR-0045). Between the `--tier` flag and `config.toml` in
/// precedence: handy for a one-off run without editing either.
pub const TIER_ENV: &str = "LMV_TIER";

/// Which of the three pin sources decided the tier, so a surprising tier is
/// traceable to what set it rather than being a mystery on stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierSource {
    /// `--tier <name>` on the command line.
    Flag,
    /// The [`TIER_ENV`] environment variable.
    Env,
    /// `config.toml`'s `[quality] tier`.
    Config,
    /// Nothing pinned it: the engine resolves the rich tier and the frame-time
    /// governor may demote it once (ADR-0045).
    Auto,
}

impl TierSource {
    /// How to name this source in a log line.
    pub fn as_str(self) -> &'static str {
        match self {
            TierSource::Flag => "--tier",
            TierSource::Env => TIER_ENV,
            TierSource::Config => "config.toml",
            TierSource::Auto => "auto",
        }
    }
}

/// The tier [`TIER_ENV`] pins, or `None` when it is unset or empty — empty is
/// treated as unset, the same rule `LMV_PRESET_DIR` follows, because an
/// exported-but-blank variable is a shell artifact rather than a choice.
///
/// `Err` on an unparseable value. It has to be *reported* rather than silently
/// defaulted — an operator who typed `LMV_TIER=rch` is otherwise convinced they
/// pinned a tier they did not — but the app degrades to the next source down
/// rather than refusing to start (NFR 10).
pub fn tier_env() -> Result<Option<lmv_core::render::Tier>, String> {
    parse_tier_env(std::env::var_os(TIER_ENV))
}

/// [`tier_env`]'s rule as a pure function of the raw value.
fn parse_tier_env(raw: Option<OsString>) -> Result<Option<lmv_core::render::Tier>, String> {
    let Some(raw) = raw.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let text = raw.to_string_lossy();
    lmv_core::render::Tier::from_name(&text)
        .map(Some)
        .ok_or_else(|| format!("{TIER_ENV}=`{text}`: expected `floor` or `rich`"))
}

/// Resolve the quality-tier pin from the three sources, highest precedence
/// first: `--tier`, then [`TIER_ENV`], then `config.toml`. `None` means auto —
/// the renderer resolves the rich tier and the governor may demote it once.
///
/// Pure: every source arrives already parsed, so the precedence rule is testable
/// without touching process-global environment state, and a source that failed
/// to parse is simply `None` here — it does not swallow the sources below it.
pub fn resolve_tier(
    flag: Option<lmv_core::render::Tier>,
    env: Option<lmv_core::render::Tier>,
    config: Option<lmv_core::render::Tier>,
) -> (Option<lmv_core::render::Tier>, TierSource) {
    match (flag, env, config) {
        (Some(tier), _, _) => (Some(tier), TierSource::Flag),
        (None, Some(tier), _) => (Some(tier), TierSource::Env),
        (None, None, Some(tier)) => (Some(tier), TierSource::Config),
        (None, None, None) => (None, TierSource::Auto),
    }
}

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

    /// The tier pin's precedence, one source at a time: flag beats env beats
    /// config beats auto, and each source is reported as what set it.
    #[test]
    fn the_tier_pin_resolves_highest_precedence_first() {
        use lmv_core::render::Tier;

        // The flag wins even when both lower sources disagree with it.
        assert_eq!(
            resolve_tier(Some(Tier::Floor), Some(Tier::Rich), Some(Tier::Rich)),
            (Some(Tier::Floor), TierSource::Flag)
        );
        // Env beats config.
        assert_eq!(
            resolve_tier(None, Some(Tier::Floor), Some(Tier::Rich)),
            (Some(Tier::Floor), TierSource::Env)
        );
        // Config is the last pin.
        assert_eq!(
            resolve_tier(None, None, Some(Tier::Rich)),
            (Some(Tier::Rich), TierSource::Config)
        );
        // Nothing set: auto, which the renderer resolves as rich + governor.
        assert_eq!(resolve_tier(None, None, None), (None, TierSource::Auto));
    }

    /// The env var parses both tiers, treats empty as unset, and **reports** a
    /// junk value rather than defaulting it — silently ignoring `LMV_TIER=rch`
    /// would leave the operator convinced they had pinned a tier they had not.
    #[test]
    fn the_tier_env_var_parses_or_reports() {
        use lmv_core::render::Tier;

        assert_eq!(
            parse_tier_env(Some(OsString::from("floor"))),
            Ok(Some(Tier::Floor))
        );
        assert_eq!(
            parse_tier_env(Some(OsString::from("RICH"))),
            Ok(Some(Tier::Rich))
        );
        assert_eq!(parse_tier_env(None), Ok(None));
        assert_eq!(parse_tier_env(Some(OsString::new())), Ok(None));

        let err = parse_tier_env(Some(OsString::from("ultra"))).expect_err("`ultra` is not a tier");
        assert!(err.contains(TIER_ENV), "{err}");
        assert!(err.contains("ultra"), "{err}");

        // A junk value does not swallow the config pin below it: it resolves to
        // `None` here, and `resolve_tier` then falls through to the config.
        assert_eq!(
            resolve_tier(None, None, Some(Tier::Floor)),
            (Some(Tier::Floor), TierSource::Config)
        );
    }
}
