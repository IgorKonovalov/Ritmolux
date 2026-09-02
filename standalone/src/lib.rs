//! Host-side helpers shared by the `lmv` binary and the `shot` example: the
//! per-OS path resolution below, plus [`shot`]'s pure CLI helpers (which live
//! here so `cargo test` actually runs their tests — an `examples/` target's
//! `#[test]` does not).
//!
//! [`osc`] is here for the second half of that same reason: it is the binary's
//! alone, but its encoder is a pure function with an exact contract, and a
//! library module is where those tests run.
//!
//! ## Preset-directory resolution
//!
//! Hand-copying the per-OS preset directory into `src/main.rs` and
//! `examples/shot.rs` lets the two copies drift, which silently breaks the one
//! invariant the `RLX_PRESET_DIR` override rests on — the app and `shot` must
//! resolve the *same* directory (ADR-0014). This module is that single source.
//! It is host-only by design: `%APPDATA%` / `HOME` / `XDG` conventions are a
//! shell concern and never leak into the source-agnostic core.
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

// Resolving one operator-supplied GPU name into the renderer's adapter and
// the sender's, each against its own roster (ADR-0146). Not feature-gated:
// the renderer half applies with or without a video-out, and the sender half
// takes the roster as a slice so it compiles and tests without the SDK.
pub mod gpu;
pub mod osc;
pub mod rss;
pub mod shot;
// The Spout video-out (ADR-0125). Behind a default-off feature AND a Windows
// cfg, so nothing about a normal build reaches it: the C++ shim it binds is
// compiled by build.rs under the same two conditions.
#[cfg(all(feature = "spout", windows))]
pub mod spout;

/// Per-user application directory name, used under the OS data root for the
/// shared preset directory, the diagnostics log, and `config.toml`.
pub const APP_DIR_NAME: &str = "Ritmolux";

/// The directory name [`migrate_app_dir_in`] reads from. A machine that has run
/// an earlier build keeps its presets, `config.toml` and diagnostics log under
/// this name, so resolving [`APP_DIR_NAME`] without carrying them across would
/// look exactly like data loss.
///
/// `plugin-foobar/foo_ritmolux.cpp` resolves the same per-user path on its own
/// and must be kept in step with [`APP_DIR_NAME`]; it does not migrate.
pub const LEGACY_APP_DIR_NAME: &str = "light-music-visualizer";

/// Environment variable naming a preset directory that overrides the per-user
/// default (ADR-0014). Set it to the repo's `presets/` for the edit-live loop,
/// or to any folder to run a custom preset library.
pub const PRESET_DIR_ENV: &str = "RLX_PRESET_DIR";

/// Environment variable pinning the quality tier — `floor` or `rich`
/// (Plan 0044 / ADR-0045). Between the `--tier` flag and `config.toml` in
/// precedence: handy for a one-off run without editing either.
pub const TIER_ENV: &str = "RLX_TIER";

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
/// treated as unset, the same rule `RLX_PRESET_DIR` follows, because an
/// exported-but-blank variable is a shell artifact rather than a choice.
///
/// `Err` on an unparseable value. It has to be *reported* rather than silently
/// defaulted — an operator who typed `RLX_TIER=rch` is otherwise convinced they
/// pinned a tier they did not — but the app degrades to the next source down
/// rather than refusing to start (NFR 10).
pub fn tier_env() -> Result<Option<rlx_core::render::Tier>, String> {
    parse_tier_env(std::env::var_os(TIER_ENV))
}

/// [`tier_env`]'s rule as a pure function of the raw value.
fn parse_tier_env(raw: Option<OsString>) -> Result<Option<rlx_core::render::Tier>, String> {
    let Some(raw) = raw.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let text = raw.to_string_lossy();
    rlx_core::render::Tier::from_name(&text)
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
    flag: Option<rlx_core::render::Tier>,
    env: Option<rlx_core::render::Tier>,
    config: Option<rlx_core::render::Tier>,
) -> (Option<rlx_core::render::Tier>, TierSource) {
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
    /// `RLX_PRESET_DIR` pointed here — user-owned, so we never seed into it.
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

    /// True when `RLX_PRESET_DIR` supplied the directory.
    pub fn is_override(&self) -> bool {
        matches!(self, PresetDir::Override(_))
    }
}

/// Resolve the preset directory: `RLX_PRESET_DIR` wins when set to a non-empty
/// path, otherwise the per-OS default under [`preset_data_root`], otherwise
/// [`PresetDir::Unresolved`].
///
/// Deliberately free of side effects: [`migrate_app_dir`] is a separate call, so
/// that resolving a path in a test can never move a directory on the machine
/// running it.
pub fn resolve_preset_dir() -> PresetDir {
    resolve_preset_dir_from(std::env::var_os(PRESET_DIR_ENV), preset_data_root())
}

/// What [`migrate_app_dir_in`] did, so the caller can report it once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppDirMigration {
    /// No directory under [`LEGACY_APP_DIR_NAME`]; nothing to carry across.
    NotNeeded,
    /// The legacy directory now lives under [`APP_DIR_NAME`].
    Moved { from: PathBuf, to: PathBuf },
    /// Both names exist. [`APP_DIR_NAME`] is used and the legacy directory is
    /// left untouched — merging two libraries would be a guess, and discarding
    /// either would be the data loss this migration exists to avoid.
    BothPresent { legacy: PathBuf },
    /// The rename failed. The legacy directory is still there and still
    /// readable by hand; the app carries on with a fresh directory rather than
    /// refusing to start (NFR §10).
    Failed { from: PathBuf, error: String },
}

/// Carry a per-user directory left under [`LEGACY_APP_DIR_NAME`] across to
/// [`APP_DIR_NAME`], as a function of `root` so it is testable without touching
/// the real `%APPDATA%`.
///
/// A plain rename, not a copy: both names sit directly under the same data root,
/// so the move is within one volume and cannot half-succeed.
pub fn migrate_app_dir_in(root: &Path) -> AppDirMigration {
    let legacy = root.join(LEGACY_APP_DIR_NAME);
    let current = root.join(APP_DIR_NAME);
    if !legacy.is_dir() {
        return AppDirMigration::NotNeeded;
    }
    if current.exists() {
        return AppDirMigration::BothPresent { legacy };
    }
    match std::fs::rename(&legacy, &current) {
        Ok(()) => AppDirMigration::Moved {
            from: legacy,
            to: current,
        },
        Err(error) => AppDirMigration::Failed {
            from: legacy,
            error: error.to_string(),
        },
    }
}

/// Run [`migrate_app_dir_in`] against the real data root, at most once per
/// process. Returns [`AppDirMigration::NotNeeded`] on every call after the
/// first, and when no data root resolves at all.
pub fn migrate_app_dir() -> AppDirMigration {
    static DONE: std::sync::Once = std::sync::Once::new();
    let mut outcome = AppDirMigration::NotNeeded;
    DONE.call_once(|| {
        if let Some(root) = preset_data_root() {
            outcome = migrate_app_dir_in(&root);
        }
    });
    outcome
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

    /// A scratch data root under the OS temp dir, emptied first so a previous
    /// run's leftovers cannot decide the outcome. Named per test, because these
    /// four run concurrently.
    fn scratch_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("rlx-appdir-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch root");
        root
    }

    /// A directory under `root` holding one file, so the assertions can tell a
    /// carried-across directory from a freshly created empty one.
    fn seed_dir(root: &Path, name: &str, marker: &str) -> PathBuf {
        let dir = root.join(name).join("presets");
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(dir.join("marker.toml"), marker).expect("write marker");
        root.join(name)
    }

    #[test]
    fn a_legacy_directory_alone_is_carried_across_and_nothing_is_left_behind() {
        let root = scratch_root("moved");
        seed_dir(&root, LEGACY_APP_DIR_NAME, "legacy");

        let outcome = migrate_app_dir_in(&root);

        assert_eq!(
            outcome,
            AppDirMigration::Moved {
                from: root.join(LEGACY_APP_DIR_NAME),
                to: root.join(APP_DIR_NAME),
            }
        );
        // The data arrived, contents intact...
        assert_eq!(
            std::fs::read_to_string(root.join(APP_DIR_NAME).join("presets/marker.toml")).unwrap(),
            "legacy"
        );
        // ...and nothing is left at the old name, which is what stops the next
        // run from seeing both and taking the BothPresent arm forever.
        assert!(!root.join(LEGACY_APP_DIR_NAME).exists());
    }

    #[test]
    fn both_present_leaves_the_legacy_directory_untouched_and_reads_the_new_one() {
        let root = scratch_root("both");
        seed_dir(&root, LEGACY_APP_DIR_NAME, "legacy");
        seed_dir(&root, APP_DIR_NAME, "current");

        let outcome = migrate_app_dir_in(&root);

        assert_eq!(
            outcome,
            AppDirMigration::BothPresent {
                legacy: root.join(LEGACY_APP_DIR_NAME),
            }
        );
        // Neither side was written: the current directory still reads as itself,
        // and the legacy one still holds every byte it held.
        assert_eq!(
            std::fs::read_to_string(root.join(APP_DIR_NAME).join("presets/marker.toml")).unwrap(),
            "current"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(LEGACY_APP_DIR_NAME).join("presets/marker.toml"))
                .unwrap(),
            "legacy"
        );
    }

    #[test]
    fn neither_present_creates_nothing_and_resolves_exactly_as_before() {
        let root = scratch_root("fresh");

        assert_eq!(migrate_app_dir_in(&root), AppDirMigration::NotNeeded);

        // A migration that finds nothing must not create the directory itself —
        // seeding is the app's job, and an empty directory here would suppress it.
        assert!(!root.join(APP_DIR_NAME).exists());
        assert!(!root.join(LEGACY_APP_DIR_NAME).exists());
        assert_eq!(
            resolve_preset_dir_from(None, Some(root.clone())),
            PresetDir::Default(root.join(APP_DIR_NAME).join("presets"))
        );
    }

    #[test]
    fn the_override_wins_over_a_pending_migration() {
        let root = scratch_root("override");
        seed_dir(&root, LEGACY_APP_DIR_NAME, "legacy");

        // A migration is pending — not yet run — and the override still decides.
        assert_eq!(
            resolve_preset_dir_from(Some(OsString::from("/repo/presets")), Some(root.clone())),
            PresetDir::Override(PathBuf::from("/repo/presets"))
        );

        // And it still decides after the migration has moved the directory, so
        // the override's precedence does not depend on when the move happened.
        assert!(matches!(
            migrate_app_dir_in(&root),
            AppDirMigration::Moved { .. }
        ));
        assert_eq!(
            resolve_preset_dir_from(Some(OsString::from("/repo/presets")), Some(root)),
            PresetDir::Override(PathBuf::from("/repo/presets"))
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
        use rlx_core::render::Tier;

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
    /// junk value rather than defaulting it — silently ignoring `RLX_TIER=rch`
    /// would leave the operator convinced they had pinned a tier they had not.
    #[test]
    fn the_tier_env_var_parses_or_reports() {
        use rlx_core::render::Tier;

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
