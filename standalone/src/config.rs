//! Per-user operator config for the live-show standalone (Plan 0009).
//!
//! A small `config.toml` under the same per-user app dir the presets live in
//! (`%APPDATA%\light-music-visualizer\` on Windows). Read once at startup and
//! written back whenever a hotkey changes a choice, so a stage setup survives a
//! restart. Only the fields the live-show features need — the full
//! settings-persistence UX stays a later roadmap item.
//!
//! Every field is `#[serde(default)]`, so a missing file, a missing section, or
//! an unknown extra key all degrade to the built-in defaults rather than crash
//! (NFR section 10 "degrade, never crash"). Later phases grow this schema
//! (`[input]`, `[rotate]`); keep additions default-able for the same reason.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The whole operator config. `#[serde(default)]` on the container fills in any
/// section the file omits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub output: Output,
    pub input: Input,
    pub rotate: Rotate,
    pub quality: Quality,
    pub hud: Hud,
}

/// `[hud]` — the on-canvas furniture the shell draws over the show (Plan 0096).
///
/// Separate from `[output]` because it is about what is *painted*, not about
/// which screen the window opens on. Two keys: the corner preset name and the
/// now-playing banner the second one took, as ADR-0110 expected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Hud {
    /// Draw the active preset's name in the top-left corner. `true` is the
    /// pre-Plan-0096 behavior, so an existing config with no `[hud]` section
    /// keeps what it had. Even when on, the name yields to a modal and to the
    /// F3 panel — this switch is "never show it", not "show it always".
    pub preset_name: bool,
    /// Announce the current track in the lower-left corner when it changes
    /// (Plan 0097). `true` because the banner is transient by construction — it
    /// clears itself after a few seconds — so the default cannot clutter a show
    /// the way a persistent line would. Off means no track ever reaches the
    /// core, not a banner drawn transparent.
    pub now_playing: bool,
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            preset_name: true,
            now_playing: true,
        }
    }
}

/// `[quality]` — the render quality tier (Plan 0044 / ADR-0045).
///
/// Persisted because a pin is a property of the *machine*, not of a run: an
/// operator who has decided their iGPU wants the floor should not have to pass
/// `--tier floor` at every launch. `--tier` and `LMV_TIER` still win over this,
/// in that order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Quality {
    /// Which tier to pin, or `auto` (the default) to let the engine resolve the
    /// rich tier and demote it if the frame time says so.
    pub tier: TierChoice,
}

/// A config-file tier choice — the two real tiers plus "let the engine decide".
/// Serializes as the kebab-case strings the config uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TierChoice {
    /// No pin: the engine resolves the rich tier and the governor may demote it.
    #[default]
    Auto,
    /// Pin the iGPU floor.
    Floor,
    /// Pin the rich tier — the governor never demotes a pin.
    Rich,
}

impl TierChoice {
    /// The pin this choice represents, or `None` for `auto`.
    pub fn tier(self) -> Option<lmv_core::render::Tier> {
        match self {
            TierChoice::Auto => None,
            TierChoice::Floor => Some(lmv_core::render::Tier::Floor),
            TierChoice::Rich => Some(lmv_core::render::Tier::Rich),
        }
    }
}

/// `[input]` — where audio comes from: loopback of whatever is playing, or a
/// line-in / audio-interface capture device (Plan 0009 Phase 2, Windows-first).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    /// Loopback of a render device, or direct capture of an input device.
    pub mode: InputMode,
    /// Friendly device name to capture. `"default"` (or a name that matches no
    /// active endpoint) falls back to the default endpoint of the selected
    /// mode's dataflow.
    pub device: String,
}

impl Default for Input {
    fn default() -> Self {
        // Loopback of the default render device — the pre-Plan-0009 behavior, so
        // an existing user with no `[input]` section keeps what they had.
        Self {
            mode: InputMode::Loopback,
            device: "default".to_owned(),
        }
    }
}

/// The capture path. Serializes as the kebab-case strings the config uses
/// (`"loopback"` / `"line-in"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputMode {
    /// Tap a render device (what the system is playing).
    #[default]
    Loopback,
    /// Capture an input device (line-in from an interface).
    LineIn,
}

impl InputMode {
    /// The kebab-case word this mode serializes as. One source for the config
    /// file, the `--input` flag, the startup line and the settings row, so what
    /// the operator reads and what the file holds are the same string.
    pub fn as_str(self) -> &'static str {
        match self {
            InputMode::Loopback => "loopback",
            InputMode::LineIn => "line-in",
        }
    }

    /// Parse the kebab-case word, or `None` when it names no mode.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "loopback" => Some(InputMode::Loopback),
            "line-in" => Some(InputMode::LineIn),
            _ => None,
        }
    }
}

/// `[rotate]` — the scene director's auto-rotate policy (Plan 0009 Phase 3;
/// defaults revised by ADR-0027 / Plan 0026).
///
/// **Hold one scene by default.** Out of the box (no `config.toml`) `auto` is
/// `false`, so the app stays on a single scene until the operator opts in — the
/// `A` hotkey (`toggle_auto`) live, or `auto = true` in the config. Manual
/// `Space` next-scene works either way.
///
/// **Calm cadence when auto is on.** The defaults favour a mostly-predictable,
/// timer-led rotation rather than a frantic one: a steady passage holds to the
/// `max_dwell_secs` cap (90 s), never rotating before `min_dwell_secs` (20 s).
/// An energy *drop* can still land a change early, but only well past the min
/// dwell (a softened gate, ~37.5 s at the default), so it can't flip scenes every
/// few seconds; a track-change boundary can nudge rotation in on the same dwell.
///
/// Dwell bounds are whole seconds (integers in the config, per the data shape),
/// converted to the director's internal float clock at construction. Every field
/// is `#[serde(default)]`, so an existing `config.toml` that pins these values
/// keeps its behaviour — the revised defaults reach only a fresh install.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotate {
    /// Auto-rotate on the dwell timer when true; manual-only (`Space`) when off.
    /// Defaults to `false` (ADR-0027): a fresh install holds one scene until the
    /// operator opts into rotation via the `toggle_auto` hotkey or `auto = true`.
    pub auto: bool,
    /// Never rotate sooner than this many seconds after the last change.
    /// Defaults to 20 s (was 8; ADR-0027).
    pub min_dwell_secs: u32,
    /// Always rotate by this many seconds even through a steady passage.
    /// Defaults to 90 s (was 40; ADR-0027).
    pub max_dwell_secs: u32,
    /// Let the experimental track-change novelty signal nudge rotation (wired in
    /// Phase 4). On by default but clearly experimental.
    pub track_change: bool,
}

impl Default for Rotate {
    fn default() -> Self {
        Self {
            auto: false,
            min_dwell_secs: 20,
            max_dwell_secs: 90,
            track_change: true,
        }
    }
}

/// `[output]` — which display, and whether to open borderless-fullscreen on it.
/// The derived defaults (`display = 0`, no name, `fullscreen = false`) are the
/// windowed first-run fallback.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Output {
    /// Target monitor index — the fallback when no `display_name` matches.
    pub display: usize,
    /// Preferred monitor identity, matched by name *before* the raw index:
    /// winit's monitor ordering can shift across boot/hotplug, so a stored
    /// index alone may point at the wrong screen (plan Risks). Empty/unset means
    /// "use the index".
    pub display_name: Option<String>,
    /// Open borderless-fullscreen on the target display when true; windowed
    /// otherwise. Default false, so a first run with no config is windowed.
    pub fullscreen: bool,
}

impl Config {
    /// Load config from `path`, degrading to the default on any problem: a
    /// missing file is the normal first-run case (silent); a malformed file is
    /// noted to stderr but still yields the windowed default rather than a
    /// crash (NFR section 10).
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("config {}: {err}; using defaults", path.display());
                    Config::default()
                }
            },
            // Missing file: first run. Any other read error also degrades quietly
            // to defaults — a config we can't read must never block the show.
            Err(_) => Config::default(),
        }
    }

    /// Write the config back to `path` (best-effort), creating the parent
    /// directory if needed. A serialize or write failure is logged and
    /// otherwise ignored — a persistence miss must not crash a live show.
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(err) = std::fs::write(path, text) {
                    eprintln!("could not write config {}: {err}", path.display());
                }
            }
            Err(err) => eprintln!("could not serialize config: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    /// **An existing `config.toml` predates `[hud]`**, so the section missing
    /// entirely has to mean today's behavior rather than a parse failure — the
    /// degrade-never-crash rule this whole module is built on (NFR 10).
    #[test]
    fn a_config_without_a_hud_section_keeps_the_name_on() {
        let config: Config = toml::from_str("[output]\nfullscreen = true\n")
            .expect("a config with no [hud] section must still parse");
        assert!(config.hud.preset_name);
        assert!(config.hud.now_playing);
    }

    /// **A `[hud]` section written before Plan 0097** has `preset_name` and
    /// nothing else. The new key has to default rather than fail the section
    /// that already exists — the same rule the missing-section case above
    /// asserts, one level down.
    #[test]
    fn a_hud_section_without_the_banner_key_keeps_the_banner_on() {
        let config: Config = toml::from_str("[hud]\npreset_name = false\n")
            .expect("a [hud] section predating now_playing must still parse");
        assert!(!config.hud.preset_name, "the key that was there must hold");
        assert!(
            config.hud.now_playing,
            "the key that was not must default on"
        );
    }

    /// The operator's "off" survives the write/read the settings row performs —
    /// which is what makes the choice outlive a restart.
    #[test]
    fn the_preset_name_choice_round_trips() {
        let mut config = Config::default();
        config.hud.preset_name = false;
        let text = toml::to_string_pretty(&config).expect("config serializes");
        let back: Config = toml::from_str(&text).expect("its own output parses");
        assert!(
            !back.hud.preset_name,
            "the off choice did not survive a save"
        );
    }

    /// The same guarantee for the banner: the settings row is only "survives a
    /// restart" if the write/read round-trips.
    #[test]
    fn the_now_playing_choice_round_trips() {
        let mut config = Config::default();
        config.hud.now_playing = false;
        let text = toml::to_string_pretty(&config).expect("config serializes");
        let back: Config = toml::from_str(&text).expect("its own output parses");
        assert!(
            !back.hud.now_playing,
            "the off choice did not survive a save"
        );
    }
}
