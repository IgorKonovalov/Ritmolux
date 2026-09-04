//! The preset directory: resolving it at launch, seeding the curated set into
//! it on first run, and watching it for edits while the show runs.
//!
//! A directory that yields at least one preset **replaces** the embedded set —
//! the answer is one list or the other and never their union, and every reader
//! here follows that same rule so two of them cannot disagree about what the
//! launch will load.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rlx_core::render::Renderer;
use standalone::{PRESET_DIR_ENV, PresetDir, resolve_preset_dir};

use crate::app_state::warn_cap_overflow;

/// How often to re-scan the preset directory for edits. Tight enough that an
/// edit to a `.toml` reads as immediate while authoring (ADR-0014); the scan
/// itself is a `read_dir` + mtime pass, negligible beside a rendered frame.
pub(crate) const PRESET_POLL: Duration = Duration::from_millis(150);

/// The preset directory to load and poll, seeding the curated set only when it
/// is the per-user default — an `RLX_PRESET_DIR` override names a directory the
/// user owns (typically the repo's version-controlled `presets/`), so writing
/// our copies into it would be a surprise (ADR-0014). Returns an empty path if
/// nothing resolves, so the caller keeps the renderer's embedded defaults
/// (degrade, never crash — NFR 10).
pub(crate) fn startup_preset_dir() -> PathBuf {
    match resolve_preset_dir() {
        PresetDir::Override(dir) => {
            eprintln!(
                "{PRESET_DIR_ENV} set: reading presets from {}",
                dir.display()
            );
            dir
        }
        PresetDir::Default(dir) => {
            seed_preset_dir(&dir);
            dir
        }
        PresetDir::Unresolved => {
            eprintln!("could not resolve a per-user data directory; keeping embedded presets");
            PathBuf::new()
        }
    }
}

/// The preset names this launch will end up with, resolved **without seeding
/// and without printing**, so `--preset` can be judged before a window exists.
///
/// Mirrors `reload_presets`'s own rule rather than approximating it: a
/// directory that yields at least one preset **replaces** the embedded set, so
/// the answer is one list or the other and never their union. A first run has
/// not seeded yet and reads as embedded, which is the same set seeding is about
/// to write.
///
/// The directory is read twice on a launch that uses this — once here and once
/// in `AppState::new`. That is a few dozen TOML parses on a startup path, and
/// it buys a refusal that costs the operator no window.
pub(crate) fn startup_preset_names() -> Vec<String> {
    let dir = match resolve_preset_dir() {
        PresetDir::Override(dir) | PresetDir::Default(dir) => dir,
        PresetDir::Unresolved => PathBuf::new(),
    };
    let from_dir = rlx_core::preset::load_dir(&dir).presets;
    let set = if from_dir.is_empty() {
        rlx_core::preset::default_presets()
    } else {
        from_dir
    };
    set.into_iter().map(|preset| preset.name).collect()
}

/// Seed the embedded curated set into `dir` on first run. An unresolved
/// (empty) path or a seeding error is logged and otherwise ignored — the
/// renderer's embedded defaults remain (degrade, never crash — NFR 10).
pub(crate) fn seed_preset_dir(dir: &Path) {
    if dir.as_os_str().is_empty() {
        return;
    }
    match rlx_core::preset::seed_dir(dir) {
        Ok(0) => {}
        Ok(n) => eprintln!("seeded {n} curated preset(s) into {}", dir.display()),
        Err(err) => eprintln!("could not seed presets into {}: {err}", dir.display()),
    }
}

/// A cheap change signature for the preset directory: the newest `.toml` mtime
/// (nanoseconds) and the file count. Any edit bumps an mtime; add/remove
/// changes the count. `None` if the directory can't be read.
pub(crate) fn dir_signature(dir: &Path) -> Option<(u128, usize)> {
    let mut latest = 0u128;
    let mut count = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            count += 1;
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified())
                && let Ok(since) = modified.duration_since(std::time::UNIX_EPOCH)
            {
                latest = latest.max(since.as_nanos());
            }
        }
    }
    Some((latest, count))
}

/// Load presets from `dir` and, if any compiled, install them on the renderer.
/// Malformed files are reported to stderr; a directory with no valid presets
/// leaves the renderer's current set (embedded defaults or last good) in place.
/// Non-fatal warnings (an unknown parameter name — usually a typo) are printed
/// too: the preset still loads and renders, and the mistake is not silent
/// (ADR-0020).
pub(crate) fn reload_presets(renderer: &mut Renderer, dir: &Path) {
    let report = rlx_core::preset::load_dir(dir);
    for (path, err) in &report.errors {
        eprintln!("preset {}: {err}", path.display());
    }
    for (path, warning) in &report.warnings {
        eprintln!("preset {}: warning: {warning}", path.display());
    }
    if report.presets.is_empty() {
        if !report.errors.is_empty() {
            eprintln!("no valid presets in {}; keeping current set", dir.display());
        }
    } else {
        eprintln!(
            "loaded {} preset(s) from {}",
            report.presets.len(),
            dir.display()
        );
        renderer.set_presets(report.presets);
        warn_cap_overflow(renderer);
    }
}
