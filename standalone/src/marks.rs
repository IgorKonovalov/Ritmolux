//! Per-user preset marks: **favourite** and **hidden**, keyed by preset name
//! (ADR-0228).
//!
//! A file of its own beside `config.toml`, under the same per-user app directory
//! the presets live in. It is *user state* rather than settings: it grows, it is
//! edited from a hotkey and from the browser rather than from the settings menu,
//! and its two keys are sets rather than scalars — so it does not share
//! `config.toml`'s writer.
//!
//! **The key is the preset's name.** An index is a position in one snapshot of
//! one roster (spec 0001), and the shipped set is embedded read-only (ADR-0022),
//! so there is no file to write a mark into and no stable id to write it under.
//! The direct costs are that renaming a preset loses its marks, that a mark on a
//! name no longer in any library is retained and inert, and that two libraries
//! sharing a preset name share its mark.
//!
//! **Nothing here can stop the app starting.** A file that is absent, empty or
//! malformed yields empty mark sets, exactly as [`crate::config`] does for
//! settings: user state that can refuse to launch the app is worse than user
//! state that is lost (NFR section 10).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{APP_DIR_NAME, preset_data_root};

/// The file the marks live in, beside `config.toml`.
pub const MARKS_FILE: &str = "marks.toml";

/// Resolve `marks.toml` under the per-user app dir (the same base as the presets,
/// `config.toml` and the diagnostics log). `None` if the OS data root cannot be
/// resolved — marks then apply live and are not persisted, the rule
/// `config.toml` already follows.
pub fn resolve_marks_path() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join(MARKS_FILE))
}

/// One of the two marks a preset can carry.
///
/// Two, rather than a rating: most presets are a shrug, so a scale would go
/// mostly unfilled, and every threshold on it becomes a constant somebody has to
/// defend (ADR-0228).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Promoted: rotation can be narrowed to these, and the browser can show
    /// only these.
    Favourite,
    /// "Stop showing me this" — excluded from rotation and from the browser's
    /// default view.
    ///
    /// **Not retirement.** A hidden preset still ships, still passes every gate
    /// and still appears in `shot --presets presets --report`; deleting the file
    /// at cohort cadence is ADR-0089's business.
    Hidden,
}

impl Mark {
    /// The word this mark is written as — in the file's key, on the wire and in
    /// a diagnostic line, so the three cannot disagree.
    pub fn as_str(self) -> &'static str {
        match self {
            Mark::Favourite => "favourite",
            Mark::Hidden => "hidden",
        }
    }

    /// Parse the word. An unknown one is `None` rather than a guess: the sender
    /// is a program, and a coerced mark would move the wrong set.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "favourite" => Some(Mark::Favourite),
            "hidden" => Some(Mark::Hidden),
            _ => None,
        }
    }
}

/// The whole of the user's opinion about the library.
///
/// Sorted sets, so the file is stable across writes and a diff of it is
/// readable; nothing downstream depends on the order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marks {
    /// Names marked favourite.
    pub favourite: BTreeSet<String>,
    /// Names marked hidden.
    pub hidden: BTreeSet<String>,
}

impl Marks {
    /// Read the marks from `path`, degrading to empty sets on any problem.
    ///
    /// A missing file is the ordinary first run and is silent. A file that
    /// exists and cannot be used is **reported**, because a mark set that
    /// silently emptied itself would read as marks that never landed.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            // Missing, or unreadable for a reason a running show cannot act on.
            return Marks::default();
        };
        match toml::from_str(&text) {
            Ok(marks) => marks,
            Err(err) => {
                eprintln!("preset marks {}: {err}; starting with none", path.display());
                Marks::default()
            }
        }
    }

    /// Write the marks back to `path` (best-effort), creating the parent
    /// directory if needed. A failure is reported and otherwise ignored — a
    /// persistence miss must not interrupt a live show.
    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(err) = std::fs::write(path, text) {
                    eprintln!("could not write preset marks {}: {err}", path.display());
                }
            }
            Err(err) => eprintln!("could not serialize preset marks: {err}"),
        }
    }

    /// The set `mark` names.
    pub fn set(&self, mark: Mark) -> &BTreeSet<String> {
        match mark {
            Mark::Favourite => &self.favourite,
            Mark::Hidden => &self.hidden,
        }
    }

    /// Whether `name` carries `mark`.
    pub fn is(&self, mark: Mark, name: &str) -> bool {
        self.set(mark).contains(name)
    }

    /// Put `name` in or out of `mark`'s set, returning whether anything moved.
    ///
    /// The `bool` is what keeps the file and the event stream off the no-op
    /// path: a control surface restating a mark it already set must not rewrite
    /// the file or announce a change that did not happen.
    pub fn apply(&mut self, mark: Mark, name: &str, on: bool) -> bool {
        let set = match mark {
            Mark::Favourite => &mut self.favourite,
            Mark::Hidden => &mut self.hidden,
        };
        if on {
            set.insert(name.to_owned())
        } else {
            set.remove(name)
        }
    }

    /// Flip `name`'s membership of `mark`'s set, returning the new state.
    pub fn toggle(&mut self, mark: Mark, name: &str) -> bool {
        let on = !self.is(mark, name);
        self.apply(mark, name, on);
        on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch file under the OS temp dir, removed first so a previous run's
    /// leftovers cannot decide the outcome.
    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("rlx-marks-{name}.toml"));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// **The walking skeleton's own claim**: a mark made now is a mark found
    /// after a restart. The file is the only thing between the two runs, so this
    /// is the round trip rather than a statement about the type.
    #[test]
    fn a_mark_survives_the_file() {
        let path = scratch("round-trip");
        let mut marks = Marks::default();
        assert!(marks.toggle(Mark::Favourite, "Echo Plate"));
        assert!(marks.toggle(Mark::Hidden, "Lace Grid"));
        marks.save(&path);

        // The next launch reads the file and nothing else.
        let back = Marks::load(&path);
        assert!(
            back.is(Mark::Favourite, "Echo Plate"),
            "the favourite did not survive the file"
        );
        assert!(
            back.is(Mark::Hidden, "Lace Grid"),
            "the hidden mark did not survive the file"
        );
        assert!(
            !back.is(Mark::Hidden, "Echo Plate"),
            "the two sets are independent and one leaked into the other"
        );
        assert_eq!(back, marks, "the file is a faithful copy of the sets");

        // And unmarking survives the same way, so a mark can be undone.
        let mut back = back;
        assert!(!back.toggle(Mark::Favourite, "Echo Plate"));
        back.save(&path);
        assert!(!Marks::load(&path).is(Mark::Favourite, "Echo Plate"));

        let _ = std::fs::remove_file(&path);
    }

    /// **Absent, empty and malformed all yield empty mark sets** rather than a
    /// failure — the property that keeps user state from being able to refuse to
    /// launch the app (ADR-0228).
    #[test]
    fn an_unusable_file_yields_empty_sets_rather_than_a_failure() {
        let path = scratch("absent");
        assert_eq!(
            Marks::load(&path),
            Marks::default(),
            "a first run has no file and must start with no marks"
        );

        std::fs::write(&path, "").expect("write an empty marks file");
        assert_eq!(
            Marks::load(&path),
            Marks::default(),
            "an empty file is a file with no marks in it"
        );

        std::fs::write(&path, "favourite = \"not a list\"\n").expect("write a malformed file");
        assert_eq!(
            Marks::load(&path),
            Marks::default(),
            "a malformed file must degrade to no marks, never stop the launch"
        );

        // A section this build has never heard of degrades the same way, so a
        // file written by a later build still loads the keys this one knows.
        std::fs::write(&path, "favourite = [\"Seahorse\"]\nrating = 4\n")
            .expect("write a file with an unknown key");
        let marks = Marks::load(&path);
        assert!(marks.is(Mark::Favourite, "Seahorse"));
        assert!(marks.hidden.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    /// A mark on a name no library holds is **retained and inert** — ADR-0228's
    /// stated cost, asserted so that "nothing prunes it" is a decision rather
    /// than an oversight.
    #[test]
    fn a_mark_on_a_name_no_longer_in_the_library_is_kept() {
        let path = scratch("stale");
        let mut marks = Marks::default();
        marks.toggle(Mark::Favourite, "A Preset That Was Renamed");
        marks.save(&path);

        let back = Marks::load(&path);
        assert!(
            back.is(Mark::Favourite, "A Preset That Was Renamed"),
            "a mark whose preset is gone must be kept: nothing here knows which \
             library is authoritative, so pruning would be a guess"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// `apply` reports whether it moved anything, which is what keeps a restated
    /// mark off the file-write and event paths.
    #[test]
    fn applying_a_mark_that_is_already_set_moves_nothing() {
        let mut marks = Marks::default();
        assert!(marks.apply(Mark::Favourite, "Gyre", true), "the first set");
        assert!(
            !marks.apply(Mark::Favourite, "Gyre", true),
            "setting a mark that is already set must report no change"
        );
        assert!(marks.apply(Mark::Favourite, "Gyre", false), "the clear");
        assert!(
            !marks.apply(Mark::Favourite, "Gyre", false),
            "clearing a mark that is not set must report no change"
        );
    }

    /// The word a mark is written as round-trips, so the file key, the wire
    /// argument and a diagnostic line are one string.
    #[test]
    fn every_mark_parses_back_from_its_own_word() {
        for mark in [Mark::Favourite, Mark::Hidden] {
            assert_eq!(Mark::from_name(mark.as_str()), Some(mark));
        }
        assert_eq!(Mark::from_name("favorite"), None, "no spelling guesses");
        assert_eq!(Mark::from_name(""), None);
    }
}
