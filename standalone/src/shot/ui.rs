//! `shot --ui <state>`: one named interface state composed over a fixed preset
//! frame, from **fixed data**, by the functions the window draws with.
//!
//! [`compose`] builds a state's text through [`crate::overlay`],
//! [`crate::settings`] and [`crate::console`] — the same line-building calls the
//! shell makes every frame — so a capture of a modal and the modal on screen
//! differ only in the state they were handed. What each state is handed is the
//! fixture set below: a fixed browser roster, a fixed filter string, a fixed
//! banner string and a fixed settings view. None of it reads the machine.
//!
//! **A capture, never a golden.** Text is shaped by glyphon on the machine's own
//! system sans-serif font, so the same command draws different glyphs on two
//! machines. The layout is fixed; the pixels are not.
//!
//! Pure: no GPU, no filesystem. The example renders the frame, queues
//! [`UiFrame::lines`], turns the diagnostics panel on when the state asks for it,
//! and draws a still of the scene into [`UiFrame::still`].

use rlx_core::render::now_playing::{FADE_IN_SECS, NowPlaying};
use rlx_core::render::{GridScale, Tier};

use crate::config::{GridScaleChoice, InputMode, RotateOrder, RotateSource};
use crate::console::{self, Line};
use crate::overlay::{self, OverlayKey, OverlayState, Pane, Row};
use crate::settings::{SettingsKey, SettingsState, SettingsView, TierState};

/// One interface state `--ui` can capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiState {
    /// The corner name plate, carrying a mark, with the rotation countdown.
    Hud,
    /// The preset browser, freshly opened, with the preview pane's placeholder.
    Browse,
    /// The browser narrowed by [`FIXTURE_FILTER`].
    BrowseFiltered,
    /// The browser with a picture in the preview pane.
    BrowseThumbs,
    /// The settings menu, freshly opened.
    Settings,
    /// The operator console's standing furniture: header, transport, staging.
    Console,
    /// The now-playing banner at full opacity, under the corner name plate.
    Banner,
    /// The F3 diagnostics panel and the capture line under it.
    Diagnostics,
}

impl UiState {
    /// Every state, in the order `--ui all` writes them.
    pub const ALL: [UiState; 8] = [
        UiState::Hud,
        UiState::Browse,
        UiState::BrowseFiltered,
        UiState::BrowseThumbs,
        UiState::Settings,
        UiState::Console,
        UiState::Banner,
        UiState::Diagnostics,
    ];

    /// The name `--ui` takes, which is also the PNG's file stem.
    pub fn name(self) -> &'static str {
        match self {
            UiState::Hud => "hud",
            UiState::Browse => "browse",
            UiState::BrowseFiltered => "browse-filtered",
            UiState::BrowseThumbs => "browse-thumbs",
            UiState::Settings => "settings",
            UiState::Console => "console",
            UiState::Banner => "banner",
            UiState::Diagnostics => "diagnostics",
        }
    }

    /// The state `name` spells, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.name() == name)
    }
}

/// Parse `--ui <state>[,<state>...]`, where `all` stands for every state in
/// [`UiState::ALL`] order.
///
/// A repeated state is rejected rather than written twice, the rule `--at`
/// keeps for a repeated hop; an unknown one names every legal spelling.
pub fn parse_ui(spec: &str) -> Result<Vec<UiState>, String> {
    let mut out: Vec<UiState> = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let states: Vec<UiState> = if part == "all" {
            UiState::ALL.to_vec()
        } else {
            let state = UiState::from_name(part).ok_or_else(|| {
                let known: Vec<&str> = UiState::ALL.iter().map(|s| s.name()).collect();
                format!("--ui: unknown state `{part}` ({} | all)", known.join(" | "))
            })?;
            vec![state]
        };
        for state in states {
            if out.contains(&state) {
                return Err(format!("--ui: `{}` listed twice", state.name()));
            }
            out.push(state);
        }
    }
    if out.is_empty() {
        return Err("--ui needs a state name or `all`".to_string());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The browser's roster: `(name, family, favourite, hidden)`.
///
/// Long enough to fill more than one column at both audit sizes, with names on
/// both sides of the truncation budget and a favourite and a hidden mark, so
/// every row style the browser has is on screen at once.
pub const FIXTURE_ROWS: [(&str, &str, bool, bool); 40] = [
    ("Aurora", "fragment", true, false),
    ("Aurora Veil", "fragment", false, false),
    ("Basalt Drift", "warp", false, false),
    ("Blue Hour", "fragment", false, false),
    ("Clifford Ember", "attractor", true, false),
    ("Clifford Lace", "attractor", false, false),
    ("Coral Bloom", "curve", false, false),
    ("Crystal Lattice", "cellular", false, false),
    ("De Jong Smoke", "attractor", false, false),
    ("Dune Ripple", "warp", false, true),
    ("Ember Rain", "swarm", false, false),
    ("Fern Spiral", "curve", false, false),
    ("Glass Garden", "cellular", true, false),
    ("Harmonic Rose", "curve", false, false),
    ("Ink Tide", "warp", false, false),
    ("Iris Bloom Kaleidoscope", "curve", false, false),
    ("Lantern Swarm", "swarm", false, false),
    ("Lissajous Knot", "curve", false, false),
    ("Magnetic Field Lines", "warp", false, false),
    ("Midnight Mandala", "cellular", false, false),
    ("Moss Fractal", "fragment", false, false),
    ("Nebula Core", "fragment", true, false),
    ("Northern Lights", "fragment", false, false),
    ("Orbit Trails", "swarm", false, false),
    ("Pale Fire", "fragment", false, false),
    ("Quartz Prism", "cellular", false, false),
    ("Rose Star", "curve", false, false),
    ("Salt Marsh", "warp", false, false),
    ("Silk Road", "warp", false, false),
    ("Snow Crash", "swarm", false, false),
    ("Star Mandala Bordered", "curve", false, false),
    ("Starling Murmuration", "swarm", true, false),
    ("Stardust", "swarm", false, false),
    ("Storm Cell", "fragment", false, false),
    ("Sunspot", "fragment", false, false),
    ("Tidal Lock", "attractor", false, false),
    ("Velvet Tunnel", "cellular", false, false),
    ("Violet Hour", "fragment", false, false),
    ("Wire Garden", "curve", false, false),
    ("Zenith", "attractor", false, false),
];

/// The shipped preset every state is composed over unless `--preset` names
/// another — one whose frame is neither blank nor so bright that light text is
/// unreadable on it, which is the frame an interface has to survive.
pub const FIXTURE_PRESET: &str = "Nebula";

/// The roster index the fixture show is on — the row a fresh browser opens at.
pub const FIXTURE_ACTIVE: usize = 12;

/// What `browse-filtered` types into the browser.
pub const FIXTURE_FILTER: &str = "star";

/// What `banner` announces, in the `artist - title` form both sources push.
pub const FIXTURE_BANNER: &str = "Boards of Canada - Roygbiv";

/// The capture token `diagnostics` prints, in the shape a live run's has.
pub const FIXTURE_CAPTURE_TOKEN: &str = "live WASAPI 48000/2 Speakers (fixture)";

/// Seconds left on the rotation countdown under the corner name.
pub const FIXTURE_NEXT_SECS: f32 = 42.0;

/// The name the console's staging line announces.
pub const FIXTURE_NEXT_UP: &str = "Northern Lights";

/// The dwell bounds the console's staging line reports.
pub const FIXTURE_DWELL: (u32, u32) = (20, 90);

/// The size the browser's preview still is stored at — the thumbnail cache's,
/// which the pane draws at twice its pixels.
pub const STILL_W: u32 = 160;
pub const STILL_H: u32 = 90;

/// The settings menu's values: a machine with two adapters and two displays,
/// every `[hud]` switch on, and a pinned tier.
pub fn fixture_view() -> SettingsView {
    SettingsView {
        tier: Tier::Rich,
        tier_state: TierState::Pinned,
        grid_scale: GridScale::FULL,
        grid_scale_choice: GridScaleChoice::Auto,
        auto_rotate: true,
        rotate_order: RotateOrder::Shuffled,
        rotate_source: RotateSource::All,
        favourites_marked: true,
        min_dwell_secs: FIXTURE_DWELL.0,
        max_dwell_secs: FIXTURE_DWELL.1,
        fullscreen: false,
        display_index: 0,
        display_count: 2,
        display_name: "Display 1 (fixture)".to_owned(),
        diagnostics: false,
        input_mode: InputMode::Loopback,
        input_device_index: 0,
        input_device_count: 2,
        input_device_name: "Speakers (fixture)".to_owned(),
        input_editable: true,
        preset_name: true,
        now_playing: true,
        next_rotation: true,
        console: false,
        thumbnails: true,
        adapter_index: 0,
        adapter_count: 2,
        adapter_name: "GPU (fixture)".to_owned(),
        preset_dir: "~/.local/share/Ritmolux/presets".to_owned(),
    }
}

/// [`FIXTURE_ROWS`] as the browser's rows.
fn fixture_rows() -> Vec<Row<'static>> {
    FIXTURE_ROWS
        .iter()
        .map(|&(name, family, favourite, hidden)| Row {
            name,
            family,
            favourite,
            hidden,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// What one state draws over the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrame {
    /// The text, in device pixels on the capture's surface.
    pub lines: Vec<Line>,
    /// Whether the core's diagnostics panel is on.
    pub diagnostics: bool,
    /// Where a still of the scene is drawn, for a state that shows one.
    pub still: Option<Pane>,
}

/// Compose `state` for a `width` x `height` surface showing preset `preset`.
///
/// The frame follows the shell's own visibility rules: a modal or the
/// diagnostics panel takes the corner name plate away, and the banner leaves it.
///
/// `console` is the console **surface**, the size of the capture: the standing
/// lines are laid out at the reference geometry and shrunk by
/// [`console::scale`], as the console window's are. The real console draws a
/// letterboxed preview of the show behind them; the capture draws the scene
/// full-frame instead.
pub fn compose(state: UiState, width: f32, height: f32, preset: &str) -> UiFrame {
    let mut frame = UiFrame {
        lines: Vec::new(),
        diagnostics: false,
        still: None,
    };
    let corner = |lines: &mut Vec<Line>| {
        overlay::corner_lines(
            preset,
            true,
            false,
            overlay::next_rotation_line(Some(FIXTURE_NEXT_SECS), true),
            lines,
        );
    };
    match state {
        UiState::Hud => corner(&mut frame.lines),
        UiState::Browse | UiState::BrowseFiltered | UiState::BrowseThumbs => {
            let rows = fixture_rows();
            let mut browse = OverlayState::new();
            let probe = overlay::layout(rows.len(), 0, width, height);
            browse.handle_key(OverlayKey::Toggle, &rows, FIXTURE_ACTIVE, &probe);
            if state == UiState::BrowseFiltered {
                for c in FIXTURE_FILTER.chars() {
                    browse.handle_key(OverlayKey::Char(c), &rows, FIXTURE_ACTIVE, &probe);
                }
            }
            let visible = browse.visible(&rows);
            let layout = overlay::layout(visible.len(), browse.highlight(), width, height);
            overlay::browse_lines(&browse, &visible, &layout, &mut frame.lines);
            let highlighted = visible.get(browse.highlight()).map(|(_, row)| row.name);
            if let (Some(pane), Some(name)) = (overlay::pane(width, height), highlighted) {
                let shown = state == UiState::BrowseThumbs;
                overlay::pane_lines(&pane, name, shown, &mut frame.lines);
                frame.still = shown.then_some(pane);
            }
        }
        UiState::Settings => {
            let view = fixture_view();
            let mut settings = SettingsState::new();
            settings.handle_key(SettingsKey::Toggle, &view);
            overlay::settings_lines(&settings, &view, &mut frame.lines);
        }
        UiState::Console => {
            let mut lines =
                console::standing_lines(preset, true, Some(FIXTURE_NEXT_UP), FIXTURE_DWELL);
            console::scale_lines(&mut lines, console::scale(height));
            frame.lines = lines;
        }
        UiState::Banner => {
            corner(&mut frame.lines);
            let mut banner = NowPlaying::default();
            banner.set(FIXTURE_BANNER);
            banner.advance(FADE_IN_SECS);
            for line in banner.layout(width, height).into_iter().flatten() {
                frame.lines.push(Line::new(
                    line.text.into_owned(),
                    line.x,
                    line.y,
                    line.size,
                    line.color,
                ));
            }
        }
        UiState::Diagnostics => {
            frame.diagnostics = true;
            frame
                .lines
                .push(overlay::capture_verdict_line(FIXTURE_CAPTURE_TOKEN));
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pair ADR-0037 names: a mistake keyed to the wrong axis is invisible
    /// at the first and glaring at the second.
    const SIZES: [(f32, f32); 2] = [(1920.0, 1080.0), (1280.0, 800.0)];

    #[test]
    fn all_expands_to_every_state_in_order_and_names_parse_back() {
        assert_eq!(parse_ui("all"), Ok(UiState::ALL.to_vec()));
        for state in UiState::ALL {
            assert_eq!(UiState::from_name(state.name()), Some(state));
            assert_eq!(parse_ui(state.name()), Ok(vec![state]));
        }
        assert_eq!(
            parse_ui("settings, hud"),
            Ok(vec![UiState::Settings, UiState::Hud]),
            "a list keeps the caller's order"
        );
    }

    #[test]
    fn a_bad_ui_spec_is_an_error_that_names_the_choices() {
        let err = parse_ui("brows").expect_err("a typo");
        assert!(err.contains("unknown state `brows`"), "{err}");
        assert!(
            err.contains("browse-thumbs") && err.contains("all"),
            "{err}"
        );
        assert!(
            parse_ui("hud,hud").is_err(),
            "a repeat writes one file twice"
        );
        assert!(parse_ui("all,hud").is_err(), "`all` already holds hud");
        assert!(parse_ui("").is_err());
        assert!(parse_ui(",").is_err());
    }

    /// Every state draws text, and every line starts on the surface at both
    /// audit sizes — a line whose origin is off the edge is a state the capture
    /// claims to show and does not.
    #[test]
    fn every_state_draws_text_that_starts_on_the_surface() {
        for (w, h) in SIZES {
            for state in UiState::ALL {
                let frame = compose(state, w, h, "Aurora");
                assert!(
                    !frame.lines.is_empty(),
                    "{} at {w}x{h} drew nothing",
                    state.name()
                );
                for line in &frame.lines {
                    assert!(
                        (0.0..w).contains(&line.x) && (0.0..h).contains(&line.y),
                        "{} at {w}x{h}: `{}` starts at ({}, {}), off the surface",
                        state.name(),
                        line.text,
                        line.x,
                        line.y
                    );
                    assert!(
                        line.size > 0.0,
                        "{}: `{}` has no size",
                        state.name(),
                        line.text
                    );
                }
            }
        }
    }

    /// The panel and the still belong to exactly the states that name them.
    #[test]
    fn only_diagnostics_turns_the_panel_on_and_only_thumbs_draws_a_still() {
        for (w, h) in SIZES {
            for state in UiState::ALL {
                let frame = compose(state, w, h, "Aurora");
                assert_eq!(
                    frame.diagnostics,
                    state == UiState::Diagnostics,
                    "{}",
                    state.name()
                );
                assert_eq!(
                    frame.still.is_some(),
                    state == UiState::BrowseThumbs,
                    "{} at {w}x{h}",
                    state.name()
                );
            }
        }
        let placeholder = |state| {
            compose(state, 1920.0, 1080.0, "Aurora")
                .lines
                .iter()
                .any(|line| line.text == overlay::PANE_PLACEHOLDER)
        };
        assert!(
            placeholder(UiState::Browse),
            "no picture yet, so the placeholder"
        );
        assert!(
            !placeholder(UiState::BrowseThumbs),
            "the picture replaces it"
        );
    }

    /// The filtered state is narrower than the open one and says what it typed.
    #[test]
    fn the_filtered_browser_names_its_query_and_shows_fewer_rows() {
        let open = compose(UiState::Browse, 1920.0, 1080.0, "Aurora");
        let filtered = compose(UiState::BrowseFiltered, 1920.0, 1080.0, "Aurora");
        assert_eq!(
            filtered.lines.first().map(|l| l.text.as_str()),
            Some(format!("filter: {FIXTURE_FILTER}").as_str())
        );
        assert!(
            filtered.lines.len() < open.lines.len(),
            "the filter narrowed nothing: {} lines against {}",
            filtered.lines.len(),
            open.lines.len()
        );
        // The hidden fixture row stays out of both, as it does in the app.
        assert!(!open.lines.iter().any(|l| l.text.contains("Dune Ripple")));
    }

    /// The modal states take the corner name plate away and the banner leaves
    /// it, which is the shell's rule rather than the fixture's.
    #[test]
    fn the_corner_name_follows_the_shells_visibility_rule() {
        let has_name = |state| {
            compose(state, 1920.0, 1080.0, "Aurora")
                .lines
                .iter()
                .any(|line| line.text.starts_with("Aurora  (favourite)"))
        };
        assert!(has_name(UiState::Hud));
        assert!(has_name(UiState::Banner));
        for state in [UiState::Browse, UiState::Settings, UiState::Diagnostics] {
            assert!(!has_name(state), "{} covers the corner", state.name());
        }
    }
}
