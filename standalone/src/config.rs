//! Per-user operator config for the live-show standalone (Plan 0009).
//!
//! A small `config.toml` under the same per-user app dir the presets live in
//! (`%APPDATA%\Ritmolux\` on Windows). Read once at startup and
//! written back whenever a hotkey changes a choice, so a stage setup survives a
//! restart. Only the fields the live-show features need — the full
//! settings-persistence UX stays a later roadmap item.
//!
//! Every field is `#[serde(default)]`, so a missing file, a missing section, or
//! an unknown extra key all degrade to the built-in defaults rather than crash
//! (NFR section 10 "degrade, never crash"). Later phases grow this schema
//! (`[input]`, `[rotate]`, `[osc]`); keep additions default-able for the same
//! reason.

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
    pub osc: Osc,
    pub artnet: Artnet,
    pub control: Control,
    pub console: Console,
}

/// `[artnet]` — the Art-Net fixture output (ADR-0145).
///
/// **Off by default**, for `[osc]`'s reason and one stronger: this sink does not
/// describe a show to a console, it *is* the show, and a machine that installed
/// the app must not start driving lamps.
///
/// **The fixture map is data, not structure.** Which universes exist, where
/// their datagrams go, how long each chain is and which spatial axis the
/// universe index stands for are all keys here. A rig patched differently — a
/// chain of two sticks rather than three, a universe index that runs across
/// rather than up — is a config edit, never a code change. These defaults
/// describe a **loopback receiver** rather than any real rig, so the shipped
/// file is a usable example and not somebody else's wiring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Artnet {
    /// Drive the fixtures. False out of the box.
    pub enabled: bool,
    /// Frames per second on the wire. 0 means every rendered frame.
    ///
    /// 40 is what the external probes chose and roughly 32 is what they
    /// achieved; **neither is a measurement of what the nodes prefer**, which is
    /// a hardware question nothing here can answer. It is a key rather than a
    /// constant for exactly that reason.
    pub rate_hz: u32,
    /// The flat colour every pixel holds until a look drives them.
    ///
    /// Mid grey rather than full white: it is unmistakable on every stick of
    /// every chain, which is what a first power-up is asking, without driving
    /// the whole rig at full current to ask it.
    pub color: [u8; 3],
    /// How a universe index becomes a spatial coordinate.
    pub space: ArtnetSpace,
    /// The nodes and the universes each one carries. `[[artnet.node]]` in the
    /// file, one table per controller.
    pub node: Vec<ArtnetNode>,
}

impl Default for Artnet {
    fn default() -> Self {
        Self {
            enabled: false,
            rate_hz: 40,
            color: [64, 64, 64],
            space: ArtnetSpace::default(),
            node: vec![ArtnetNode::default()],
        }
    }
}

/// `[artnet.space]` — universe index to a normalized coordinate.
///
/// Separate from the node table because it is a claim about the *structure* the
/// sticks are mounted on, where the node table is a claim about the wiring. The
/// two are confirmed by different observations and are revised independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtnetSpace {
    /// Which normalized axis the universe index drives.
    pub universe_axis: SpaceAxis,
    /// The universe index that reads 0.0 on that axis.
    ///
    /// Swapping this with `universe_max` flips the rig end for end without
    /// touching the node table, which is the repair a mis-read patch needs.
    pub universe_min: u16,
    /// The universe index that reads 1.0.
    pub universe_max: u16,
}

impl Default for ArtnetSpace {
    fn default() -> Self {
        Self {
            universe_axis: SpaceAxis::Y,
            universe_min: 0,
            universe_max: 23,
        }
    }
}

/// The normalized axis a universe index stands for. Serializes as the
/// lower-case letters the config uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpaceAxis {
    /// Universe index runs across.
    X,
    /// Universe index runs up — the rig this was written against.
    #[default]
    Y,
}

impl SpaceAxis {
    /// The letter this axis serializes as, so the config file, the startup line
    /// and any diagnostic all read the same string.
    pub fn as_str(self) -> &'static str {
        match self {
            SpaceAxis::X => "x",
            SpaceAxis::Y => "y",
        }
    }
}

/// `[[artnet.node]]` — one controller, and the universes it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtnetNode {
    /// Where its datagrams go, as `host:port`. Art-Net's port is 6454.
    pub target: String,
    /// The inclusive universe range this node carries, `[first, last]`.
    pub universes: [u16; 2],
    /// Pixels in each of this node's chains.
    ///
    /// **170, and a shorter value is the trap this key exists to make
    /// visible.** A universe is 512 channels, so 170 RGB pixels is the whole of
    /// it; a node sent fewer leaves the later sticks of the chain holding their
    /// previous frame, which presents as half the rig being broken rather than
    /// as a short frame.
    pub pixels: u16,
}

impl Default for ArtnetNode {
    fn default() -> Self {
        Self {
            target: "127.0.0.1:6454".to_owned(),
            universes: [0, 23],
            pixels: 170,
        }
    }
}

/// `[control]` — the studio control-in listener (ADR-0176).
///
/// **Off by default and loopback by default**, and the two are separate
/// promises. Off means a machine that was never asked to be driven binds no
/// port at all; loopback means one that *was* asked, and named no host, is
/// reachable only from itself. A show machine on a venue network is the case
/// both defaults are written for.
///
/// `--control <host:port>` overrides the address *and* turns the listener on for
/// that run without writing itself into the file, the same shape `--osc` follows
/// (ADR-0142).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Control {
    /// Listen for control messages. False out of the box.
    pub enabled: bool,
    /// Where to listen, as `host:port`.
    ///
    /// One above `[osc] target`'s port, so the two OSC surfaces read as a pair
    /// and a machine running both needs neither moved. Binding anywhere but
    /// loopback is the operator's explicit choice: anything that can reach the
    /// port can move a parameter, which is the feature and also the whole of the
    /// exposure.
    pub listen: String,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: "127.0.0.1:9001".to_owned(),
        }
    }
}

/// `[console]` — the operator console, a second window on a second display
/// (ADR-0143).
///
/// **Off by default**, for the reason every optional surface here is: a config
/// that has never heard of a console must produce exactly today's app — one
/// window, one surface, no intermediate render target and no extra copy per
/// frame. `--console` opens it for a run without writing itself into the file,
/// the shape `--input` / `--device` / `--osc` already follow (ADR-0142).
///
/// Its display is resolved by the **same** name-over-index rule the show's own
/// display uses, and deliberately not by a second one: winit's monitor ordering
/// is not stable across boot or hotplug, so a stored index alone can point at
/// the wrong screen, and two answers to that question would drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Console {
    /// Open the console at launch. False out of the box.
    pub enabled: bool,
    /// Preferred monitor identity for the console, matched by name before the
    /// index — exactly as `[output] display_name` is. Empty means "use the
    /// index".
    pub display_name: Option<String>,
    /// Fallback monitor index when no `display_name` matches.
    ///
    /// The default is **1, not 0**: a console's whole point is to be on a
    /// display other than the show's, and the show defaults to 0. On a
    /// single-monitor machine this falls back to the only monitor there is,
    /// which is the correct degrade rather than a failure.
    pub display: usize,

    /// The console swapchain's `desired_maximum_frame_latency`, clamped to
    /// `1..=3` by the renderer.
    ///
    /// **A pacing lever, not a picture one** — it changes nothing the console
    /// draws. At 1 the surface holds a single in-flight image, so acquiring its
    /// next texture waits for its own previous present to retire; that wait is
    /// paid on the display thread, which is also the thread the show presents
    /// on. Raising it lets the console run a frame ahead instead of making the
    /// show's loop wait for it.
    pub frame_latency: u32,

    /// Present the console every Nth output frame. `1` is every frame; `0` is
    /// read as `1`.
    ///
    /// The other end of the same question: how *often* the show's frame budget
    /// pays for a console present at all. Above 1 the console's own readout
    /// updates at a fraction of the output's rate — a sampled monitor rather
    /// than a continuous one — which is operator-visible and is why this is a
    /// key rather than a constant.
    pub present_every_n: u32,
}

impl Default for Console {
    fn default() -> Self {
        Self {
            enabled: false,
            display_name: None,
            display: 1,
            frame_latency: 1,
            present_every_n: 1,
        }
    }
}

/// `[osc]` — the lighting telemetry sink (ADR-0144).
///
/// **Off by default**, like every optional sink: a user who runs no lighting rig
/// must not have a socket bound or a datagram leaving their machine because they
/// installed the app. `--osc <host:port>` overrides the target *and* turns the
/// sink on for that run without writing itself into the file, the same shape
/// `--input` / `--device` follow (ADR-0142).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Osc {
    /// Publish telemetry. False out of the box.
    pub enabled: bool,
    /// Where to send, as `host:port`. The default names localhost so that the
    /// key is a usable example rather than a blank an operator has to guess the
    /// shape of; it is inert until `enabled` (or `--osc`) turns the sink on.
    pub target: String,
    /// Datagram sets per second. 60 tracks a 60 Hz display closely enough that
    /// the cadence is invisible; 0 means every rendered frame, whatever the
    /// frame rate. A console that throttles its own OSC input wants this lower,
    /// which is the reason it is a key rather than a constant.
    pub rate_hz: u32,
}

impl Default for Osc {
    fn default() -> Self {
        Self {
            enabled: false,
            target: "127.0.0.1:9000".to_owned(),
            rate_hz: 60,
        }
    }
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
/// `--tier floor` at every launch. `--tier` and `RLX_TIER` still win over this,
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
    pub fn tier(self) -> Option<rlx_core::render::Tier> {
        match self {
            TierChoice::Auto => None,
            TierChoice::Floor => Some(rlx_core::render::Tier::Floor),
            TierChoice::Rich => Some(rlx_core::render::Tier::Rich),
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
    use super::{Config, Console};

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

    /// **A `[hud]` section carrying `preset_name` and nothing else** — the
    /// shape Plan 0097 added `now_playing` to. The added key has to default
    /// rather than fail the section that already exists — the same rule the
    /// missing-section case above asserts, one level down.
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

    /// **A config that has never heard of a console produces exactly today's
    /// app.** Asserted on the `Default` impl and not merely observed in a fresh
    /// file, because that is what an upgrading operator gets: their file
    /// predates the section entirely, and a default that opened a second window
    /// would move a show onto a screen nobody asked for.
    #[test]
    fn the_console_is_off_by_default_and_an_absent_section_keeps_it_off() {
        assert!(
            !Console::default().enabled,
            "the Default impl must leave the console closed"
        );

        let config: Config = toml::from_str(
            "[output]
fullscreen = true
",
        )
        .expect("a config with no [console] section must still parse");
        assert!(
            !config.console.enabled,
            "an absent section opened the console"
        );
        // And the fallback index is the *second* display, not the show's: a
        // console defaulting onto display 0 would open on top of the output on
        // every multi-monitor machine.
        assert_eq!(config.console.display, 1);
        assert_eq!(config.console.display_name, None);
    }

    /// **The console's two pacing keys default to the cadence the app shipped
    /// with, and this pins them so moving either is a visible diff.**
    ///
    /// They exist to be varied — frame latency 1 or 2, present every frame or
    /// every second one, in any of the four combinations — which is exactly why
    /// the untouched combination has to be nailed down: a measurement whose
    /// baseline arm quietly ran on different defaults compares nothing. Pinned
    /// on `Default` *and* through a config that predates the keys, because an
    /// upgrading operator's file has neither.
    #[test]
    fn the_console_pacing_defaults_to_one_in_flight_image_presented_every_frame() {
        assert_eq!(
            Console::default().frame_latency,
            1,
            "the shipped swapchain depth moved"
        );
        assert_eq!(
            Console::default().present_every_n,
            1,
            "the shipped console cadence stopped being every frame"
        );

        let config: Config = toml::from_str(
            "[console]
enabled = true
display = 2
",
        )
        .expect("a [console] section predating the pacing keys must still parse");
        assert_eq!(config.console.frame_latency, 1);
        assert_eq!(config.console.present_every_n, 1);
    }

    /// **All four arms are expressible, and each survives the file.** The keys
    /// are only useful in combination, so round-tripping one of them proves
    /// nothing about the pair.
    #[test]
    fn every_console_pacing_combination_round_trips() {
        for (frame_latency, present_every_n) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
            let mut config = Config::default();
            config.console.frame_latency = frame_latency;
            config.console.present_every_n = present_every_n;

            let text = toml::to_string(&config).expect("config must serialize");
            let back: Config = toml::from_str(&text).expect("config must round-trip");

            assert_eq!(
                (back.console.frame_latency, back.console.present_every_n),
                (frame_latency, present_every_n),
                "the ({frame_latency}, {present_every_n}) arm did not survive a save"
            );
        }
    }

    /// Both `[console]` keys survive the write/read the settings row performs,
    /// which is what lets a console reopen on the display it was left on.
    #[test]
    fn the_console_choice_and_its_display_round_trip() {
        let mut config = Config::default();
        config.console.enabled = true;
        config.console.display_name = Some("DELL U2720Q".to_owned());
        config.console.display = 2;

        let text = toml::to_string_pretty(&config).expect("config serializes");
        let back: Config = toml::from_str(&text).expect("its own output parses");

        assert!(
            back.console.enabled,
            "the open choice did not survive a save"
        );
        assert_eq!(
            back.console.display_name.as_deref(),
            Some("DELL U2720Q"),
            "the named display did not survive a save, so the console would \
             reopen wherever the index happened to point"
        );
        assert_eq!(back.console.display, 2);
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

    /// **Every existing `config.toml` predates `[osc]`**, and the sink sends to
    /// the network — so the section missing has to mean *off*, not "the
    /// default target". A default that turned it on would put a datagram on the
    /// wire of every machine that upgraded.
    #[test]
    fn a_config_without_an_osc_section_leaves_the_sink_off() {
        let config: Config = toml::from_str(
            "[output]
fullscreen = true
",
        )
        .expect("a config with no [osc] section must still parse");
        assert!(!config.osc.enabled, "an absent section enabled the sink");
        assert_eq!(config.osc.rate_hz, 60);
    }

    /// A half-written `[osc]` section — the operator set a target and never
    /// touched the cadence — keeps the key it has and defaults the one it does
    /// not, rather than failing the section outright.
    #[test]
    fn a_partial_osc_section_defaults_the_rest() {
        let config: Config = toml::from_str(
            "[osc]
enabled = true
target = \"10.0.0.4:7700\"
",
        )
        .expect("a partial [osc] section must still parse");
        assert!(config.osc.enabled);
        assert_eq!(config.osc.target, "10.0.0.4:7700");
        assert_eq!(config.osc.rate_hz, 60, "the absent key did not default");
    }

    /// **An absent `[artnet]` section drives nothing.** Stronger than the OSC
    /// case it mirrors: a datagram on a telemetry port is a wasted packet, and
    /// an Art-Net datagram on a venue network is somebody's lamps.
    #[test]
    fn a_config_without_an_artnet_section_leaves_the_sink_off() {
        let config: Config = toml::from_str(
            "[output]
fullscreen = true
",
        )
        .expect("a config with no [artnet] section must still parse");
        assert!(!config.artnet.enabled, "an absent section enabled the sink");
        assert_eq!(config.artnet.rate_hz, 40);
        assert_eq!(
            config.artnet.node.len(),
            1,
            "the example node is the default"
        );
        assert_eq!(
            config.artnet.node[0].pixels, 170,
            "a short frame is the trap"
        );
    }

    /// **The whole fixture map is data, so a rig patched differently is a file
    /// edit.** Asserted by writing a map that disagrees with the default on
    /// every claim it makes — two nodes instead of one, a narrower chain, a
    /// reversed height axis — and reading back what was written.
    #[test]
    fn a_rig_patched_differently_is_described_entirely_in_the_file() {
        let config: Config = toml::from_str(
            "[artnet]
enabled = true
color = [10, 20, 30]

[artnet.space]
universe_axis = \"x\"
universe_min = 11
universe_max = 0

[[artnet.node]]
target = \"192.168.1.159:6454\"
universes = [0, 5]
pixels = 100

[[artnet.node]]
target = \"192.168.1.160:6454\"
universes = [6, 11]
pixels = 100
",
        )
        .expect("a two-node fixture map must parse");
        assert!(config.artnet.enabled);
        assert_eq!(config.artnet.color, [10, 20, 30]);
        assert_eq!(config.artnet.space.universe_axis, super::SpaceAxis::X);
        assert_eq!(
            (
                config.artnet.space.universe_min,
                config.artnet.space.universe_max
            ),
            (11, 0),
            "a reversed axis is expressible, which is how a flipped rig is fixed"
        );
        assert_eq!(config.artnet.node.len(), 2);
        assert_eq!(config.artnet.node[1].target, "192.168.1.160:6454");
        assert_eq!(config.artnet.node[1].universes, [6, 11]);
        assert_eq!(config.artnet.node[0].pixels, 100);
        assert_eq!(
            config.artnet.rate_hz, 40,
            "the absent cadence key did not default"
        );
    }

    /// The map survives the write/read `Config::save` performs, array of tables
    /// included — otherwise an operator's rig description would not outlive a
    /// hotkey that persists some unrelated choice.
    #[test]
    fn the_fixture_map_round_trips() {
        let mut config = Config::default();
        config.artnet.enabled = true;
        config.artnet.color = [1, 2, 3];
        config.artnet.node = vec![
            super::ArtnetNode {
                target: "10.0.0.1:6454".to_owned(),
                universes: [0, 3],
                pixels: 90,
            },
            super::ArtnetNode {
                target: "10.0.0.2:6454".to_owned(),
                universes: [4, 7],
                pixels: 90,
            },
        ];
        let text = toml::to_string_pretty(&config).expect("config serializes");
        let back: Config = toml::from_str(&text).expect("its own output parses");
        assert_eq!(back.artnet, config.artnet);
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
