//! The preset layer — ADR-0002 layers 1-2: TOML data binding built-in system
//! parameters to a pure expression language over the audio analysis.
//!
//! *Pure* is a statement about the evaluator, not about the surface: a
//! `[latch]` (ADR-0137) reads state the render layer holds between frames, and
//! [`expr`]'s own header says how that is arranged without the evaluator
//! learning it.
//!
//! [`expr`] compiles and evaluates expression strings; [`schema`] parses a
//! TOML preset into compiled [`Binding`]s; [`path`] parses the one thing here
//! that is not an expression, a `[path] d` string, into an authored silhouette
//! (ADR-0107). This module also loads presets in
//! bulk: [`default_presets`] embeds the shipped examples (so the C-ABI/foobar
//! path always has visuals without a preset directory), [`seed_dir`] writes the
//! embedded curated set into a per-user directory on first run (write-if-absent,
//! so a user's edits survive), and [`load_dir`] reads a directory for the
//! standalone's hot-reload path — a malformed file is reported, never fatal, so
//! the caller keeps the last good set (NFR 10). [`drift`] reads that same
//! directory against the embedded set without changing it, because seeding
//! writes if-absent and never prunes: what an operator keeps diverges, and a
//! shell has to be able to say how.

pub mod expr;
pub mod path;
pub mod schema;

use std::path::{Path, PathBuf};

pub use expr::{
    Expr, ExprError, GateFlag, GateKind, LATCH_CAP, NodeObservation, Observations,
    SATURATED_OCCUPANCY, Variables, compile,
};
pub use schema::export;
pub use schema::{
    Binding, Easing, GLOBAL_PARAMS, HoldEdge, KeyDesc, KeyKind, Latch, Layer, LayerBlend,
    LayerJoin, Preset, PresetError, PresetWarning, Roster, SystemKind, TableDesc, is_known_param,
    kind_of_param,
};

// The shipped example presets, embedded at compile time so the C-ABI/foobar
// path always has visuals without a preset directory (ADR-0006). This list is
// **generated** — `core/build.rs` globs `presets/*.toml` and emits
// `pub static EMBEDDED: &[(&str, &str)]` as `(filename, contents)` tuples,
// sorted by filename, each embedded via `include_str!` (ADR-0022). Drop a
// `.toml` in `presets/` at the repo root and rebuild — it ships, with no edit
// here and no count to bump. (See `core/build.rs` for how the entries are
// produced; they are not a literal array in this file.)
include!(concat!(env!("OUT_DIR"), "/embedded_presets.rs"));

/// Parse the embedded example presets. The shipped files are valid, so on the
/// off chance one fails it is skipped rather than panicking — the caller still
/// gets a usable set.
pub fn default_presets() -> Vec<Preset> {
    EMBEDDED
        .iter()
        .filter_map(|(_, src)| Preset::from_toml_str(src).ok())
        .collect()
}

/// Write each embedded curated preset into `dir`, creating `dir` (and any
/// missing parents) first, but **never overwriting** a file that already
/// exists — a user's edits to a seeded preset survive re-seeding. Returns how
/// many files were newly written. Idempotent: a second call on an
/// already-seeded directory writes zero.
///
/// Because seeding never clobbers, a curated preset changed in a later release
/// does **not** replace the copy a user already has on disk (a "refresh
/// curated" affordance is a follow-up, not this function's job). Errors bubble
/// up as `io::Result` so the caller can degrade to the embedded defaults rather
/// than crash (NFR 10).
pub fn seed_dir(dir: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut written = 0;
    for &(name, contents) in EMBEDDED {
        let path = dir.join(name);
        if !path.exists() {
            std::fs::write(&path, contents)?;
            written += 1;
        }
    }
    Ok(written)
}

/// How one file in a preset directory stands against the set this build ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftStatus {
    /// The filename is in [`EMBEDDED`] and the bytes are equal.
    Shipped,
    /// The filename is in [`EMBEDDED`] and the bytes differ.
    ///
    /// **An operator's edit and an older release's copy are indistinguishable
    /// here.** [`seed_dir`] never overwrites, so a preset retuned upstream
    /// leaves the copy already on disk in place, and nothing on disk records
    /// which shipped version that copy came from. Telling the two apart would
    /// need a manifest of every past release's hashes, which no build carries.
    Differs,
    /// The filename is not in [`EMBEDDED`] — an operator's own preset, or one a
    /// later release retired. Seeding never removes a file, so both linger.
    NotShipped,
}

impl DriftStatus {
    /// The word a report row prints for this status.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftStatus::Shipped => "shipped",
            DriftStatus::Differs => "differs",
            DriftStatus::NotShipped => "not shipped",
        }
    }
}

/// One `*.toml` in a preset directory, as [`drift`] judged it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftEntry {
    /// The file name alone, without the directory.
    pub file: String,
    /// Its standing against the shipped set.
    pub status: DriftStatus,
    /// The display name the file compiles to, or `None` when it does not
    /// compile — a file that cannot be read is judged by its name alone.
    pub name: Option<String>,
}

/// A display name claimed by more than one file that compiles.
///
/// Only the first file is reachable: `load_dir` sorts by filename and
/// `Renderer::select_preset_by_name` takes the first exact match, so every
/// later claimant is loaded, rotated through, and unreachable by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateName {
    /// The contested display name.
    pub name: String,
    /// The files claiming it, in filename order. Never shorter than two.
    pub files: Vec<String>,
}

impl DuplicateName {
    /// The file a lookup by this name resolves to: the first in filename order.
    pub fn winner(&self) -> &str {
        // The constructor never builds an empty claim list, so the index holds.
        &self.files[0]
    }
}

/// What a preset directory holds beyond the set this build ships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriftReport {
    /// One row per `*.toml`, in filename order.
    pub entries: Vec<DriftEntry>,
    /// Every display name claimed twice or more, in first-claim order.
    pub duplicates: Vec<DuplicateName>,
}

impl DriftReport {
    /// How many files carry a shipped name with different bytes.
    pub fn differs(&self) -> usize {
        self.count(DriftStatus::Differs)
    }

    /// How many files carry a name the shipped set does not have.
    pub fn not_shipped(&self) -> usize {
        self.count(DriftStatus::NotShipped)
    }

    fn count(&self, status: DriftStatus) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == status)
            .count()
    }

    /// Whether anything in the directory is not exactly what this build ships.
    pub fn has_drift(&self) -> bool {
        self.differs() > 0 || self.not_shipped() > 0 || !self.duplicates.is_empty()
    }

    /// The one-line summary a shell prints after seeding, or `None` when the
    /// directory is exactly the shipped set — which is the normal case and must
    /// stay silent.
    ///
    /// User-visible text: a shell prints it verbatim, after naming the subject
    /// itself. The subject is the caller's because only the caller knows which
    /// directory resolved, and because a diagnostic the shell writes must not
    /// begin with an interpolation — a parent splits its stream on the first
    /// byte (ADR-0176).
    pub fn line(&self) -> Option<String> {
        if !self.has_drift() {
            return None;
        }
        let mut parts = Vec::new();
        let differs = self.differs();
        if differs > 0 {
            parts.push(format!(
                "{differs} file(s) differ from the copy this build ships (an edit, or an \
                 older release's copy - they cannot be told apart)"
            ));
        }
        let not_shipped = self.not_shipped();
        if not_shipped > 0 {
            parts.push(format!("{not_shipped} file(s) are not in the shipped set"));
        }
        if !self.duplicates.is_empty() {
            let claimed: Vec<String> = self
                .duplicates
                .iter()
                .map(|duplicate| {
                    format!("'{}' (reached as {})", duplicate.name, duplicate.winner())
                })
                .collect();
            parts.push(format!(
                "{} display name(s) claimed by more than one file, of which only the first \
                 is reachable by name: {}",
                self.duplicates.len(),
                claimed.join(", ")
            ));
        }
        Some(format!(
            "{}. Run --list-presets for the per-file rows.",
            parts.join("; ")
        ))
    }
}

/// Judge every `*.toml` in `dir` against [`EMBEDDED`], without changing
/// anything: seeding writes if-absent and never prunes, so a directory an
/// operator keeps drifts from the shipped set in three ways at once, and this
/// is the read that names them.
///
/// Pure over the directory listing and the embedded set. A missing or
/// unreadable directory yields an empty report rather than an error, which
/// reads as "no drift" — the caller has nothing to say about a directory that
/// is not there (NFR 10).
pub fn drift(dir: &Path) -> DriftReport {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect(),
        Err(_) => return DriftReport::default(),
    };
    // Filename order, which is the order `load_dir` loads in and therefore the
    // order that decides which of two files claiming one name is reachable.
    paths.sort();

    let mut entries = Vec::new();
    // Display name -> the files claiming it, both kept in first-seen order so
    // the report is stable across runs.
    let mut claims: Vec<(String, Vec<String>)> = Vec::new();

    for path in paths {
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let file = file.to_owned();
        let source = std::fs::read_to_string(&path).ok();
        let shipped = EMBEDDED
            .iter()
            .find(|&&(name, _)| name == file)
            .map(|&(_, contents)| contents);
        // A file whose name is shipped but which cannot be read is `Differs`:
        // the bytes are not known to be equal, and claiming they are would be
        // the silent case this function exists to end.
        let status = match (shipped, source.as_deref()) {
            (Some(contents), Some(src)) if contents == src => DriftStatus::Shipped,
            (Some(_), _) => DriftStatus::Differs,
            (None, _) => DriftStatus::NotShipped,
        };
        let name = source
            .as_deref()
            .and_then(|src| Preset::from_toml_str(src).ok())
            .map(|preset| preset.name);
        if let Some(name) = &name {
            match claims.iter_mut().find(|(claimed, _)| claimed == name) {
                Some((_, files)) => files.push(file.clone()),
                None => claims.push((name.clone(), vec![file.clone()])),
            }
        }
        entries.push(DriftEntry { file, status, name });
    }

    let duplicates = claims
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(name, files)| DuplicateName { name, files })
        .collect();

    DriftReport {
        entries,
        duplicates,
    }
}

/// The outcome of loading a preset directory: the presets that compiled, in
/// filename order, plus the files that failed and the non-fatal problems found
/// in the ones that succeeded (so the caller can surface both).
pub struct LoadReport {
    /// Successfully compiled presets, sorted by filename for a stable cycle.
    pub presets: Vec<Preset>,
    /// `(path, error)` for each `.toml` that failed to read or compile.
    pub errors: Vec<(PathBuf, PresetError)>,
    /// `(path, warning)` for each non-fatal problem in a preset that **did**
    /// load — a binding naming a parameter its system does not consume
    /// (ADR-0020), among others. Surfacing these is what stops a typo from
    /// failing silently; the preset itself is in `presets` and renders normally.
    /// Each warning keeps the binding label it carries (ADR-0192).
    pub warnings: Vec<(PathBuf, PresetWarning)>,
}

/// Load every `*.toml` in `dir`, compiling each into a [`Preset`]. Missing or
/// unreadable directories yield an empty report rather than an error; a bad
/// file lands in `errors` and does not stop the others (degrade, never crash).
pub fn load_dir(dir: &Path) -> LoadReport {
    let mut presets = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
            .collect(),
        Err(_) => {
            return LoadReport {
                presets,
                errors,
                warnings,
            };
        }
    };
    paths.sort();

    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(src) => match Preset::from_toml_str(&src) {
                Ok(mut preset) => {
                    warnings.extend(preset.warnings.iter().map(|w| (path.clone(), w.clone())));
                    // Absolute, because the consumers that want it — an editor
                    // that writes the file back, a report that names it — do not
                    // share this process's working directory. `absolute` is
                    // lexical: it prepends the cwd and normalizes, touching no
                    // filesystem and, unlike `canonicalize`, producing no `\?\`
                    // prefix on Windows for a path that then has to be handed to
                    // another program. A cwd that cannot be read leaves the path
                    // as it was rather than dropping the preset (NFR 10).
                    preset.source =
                        Some(std::path::absolute(&path).unwrap_or_else(|_| path.clone()));
                    presets.push(preset);
                }
                Err(err) => errors.push((path, err)),
            },
            Err(err) => errors.push((path, PresetError::Io(err.to_string()))),
        }
    }

    LoadReport {
        presets,
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_dir_writes_all_then_nothing() {
        let dir = std::env::temp_dir().join("rlx_seed_dir_test");
        let _ = std::fs::remove_dir_all(&dir);

        // First seed into an empty dir: every embedded preset is written.
        let written = seed_dir(&dir).expect("seed into fresh temp dir");
        assert_eq!(
            written,
            EMBEDDED.len(),
            "first seed writes every embedded preset"
        );
        for &(name, _) in EMBEDDED {
            assert!(dir.join(name).exists(), "{name} was seeded");
        }

        // Second seed: write-if-absent means nothing is written and nothing is
        // clobbered.
        let again = seed_dir(&dir).expect("re-seed already-seeded dir");
        assert_eq!(
            again, 0,
            "re-seeding writes zero (idempotent, no overwrite)"
        );

        // Deleting one seeded file re-seeds only that file.
        let (victim, _) = EMBEDDED[0];
        std::fs::remove_file(dir.join(victim)).expect("remove one seeded file");
        let refill = seed_dir(&dir).expect("re-seed after deletion");
        assert_eq!(refill, 1, "only the missing file is re-written");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A directory that is exactly the shipped set says nothing.** The line is
    /// printed on every launch, so a false positive on an untouched install
    /// would be permanent noise; the formatter returning `None` is what the
    /// shell's silence rests on.
    #[test]
    fn a_freshly_seeded_dir_has_no_drift_and_no_line() {
        let dir = std::env::temp_dir().join("rlx_drift_clean_test");
        let _ = std::fs::remove_dir_all(&dir);
        seed_dir(&dir).expect("seed into fresh temp dir");

        let report = drift(&dir);
        assert_eq!(
            report.entries.len(),
            EMBEDDED.len(),
            "every seeded file is judged"
        );
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.status == DriftStatus::Shipped),
            "a seeded file that is not `Shipped`: {:?}",
            report
                .entries
                .iter()
                .find(|entry| entry.status != DriftStatus::Shipped)
        );
        assert!(
            report.duplicates.is_empty(),
            "the shipped set claims a display name twice: {:?}",
            report.duplicates
        );
        assert!(!report.has_drift());
        assert_eq!(report.line(), None, "an undrifted directory prints no line");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The three kinds of drift, each reported once.** An edited shipped file,
    /// a file the shipped set does not have, and a display name a second file
    /// claims — which is the case that costs an operator a preset, since only
    /// the first file in filename order is reachable by name.
    #[test]
    fn drift_reports_an_edit_an_extra_file_and_a_duplicate_name() {
        let dir = std::env::temp_dir().join("rlx_drift_test");
        let _ = std::fs::remove_dir_all(&dir);
        seed_dir(&dir).expect("seed into fresh temp dir");

        // An edit to a shipped file: still compiles, still its own name, and no
        // longer the bytes this build carries.
        let (edited, edited_src) = EMBEDDED[1];
        std::fs::write(
            dir.join(edited),
            format!("{edited_src}\n# an operator's note\n"),
        )
        .expect("edit one seeded file");

        // A file the shipped set does not have, carrying a display name a
        // shipped preset already claims. `zz_` sorts after every seeded name,
        // so the shipped file is the one a lookup reaches.
        let (shadowed, duplicate_src) = EMBEDDED[0];
        std::fs::write(dir.join("zz_duplicate.toml"), duplicate_src)
            .expect("write the duplicate-name file");
        let duplicated = Preset::from_toml_str(duplicate_src)
            .expect("a shipped preset compiles")
            .name;

        let report = drift(&dir);
        assert_eq!(report.differs(), 1, "exactly the edited file differs");
        assert_eq!(
            report.not_shipped(),
            1,
            "exactly the added file is not shipped"
        );
        assert_eq!(
            report.entries.len(),
            EMBEDDED.len() + 1,
            "every file is judged, the added one included"
        );
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.file == edited)
                .map(|entry| entry.status),
            Some(DriftStatus::Differs)
        );
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.file == "zz_duplicate.toml")
                .map(|entry| entry.status),
            Some(DriftStatus::NotShipped)
        );

        assert_eq!(report.duplicates.len(), 1, "one contested display name");
        let duplicate = &report.duplicates[0];
        assert_eq!(duplicate.name, duplicated);
        assert_eq!(
            duplicate.files,
            vec![shadowed.to_owned(), "zz_duplicate.toml".to_owned()]
        );
        assert_eq!(
            duplicate.winner(),
            shadowed,
            "filename order decides which file the name reaches"
        );

        assert!(report.has_drift());
        let line = report.line().expect("a drifted directory prints a line");
        assert!(
            line.contains(&duplicated) && line.contains(shadowed),
            "the line names the contested name and the file that wins it: {line}"
        );
        assert!(
            line.contains("--list-presets"),
            "the line says where the rows are: {line}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
