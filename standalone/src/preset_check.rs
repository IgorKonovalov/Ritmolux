//! `ritmolux --check <path>`: the engine's own verdict on a preset file, at a
//! position, without a GPU adapter (ADR-0190).
//!
//! The engine is what produces every diagnostic under the `engine` rule: this
//! module calls [`Preset::from_toml_str`] and reports what it said, so a
//! `--check` verdict cannot disagree with what a render would do. What it adds
//! is a **position**, which the loader does not carry:
//!
//! - A TOML syntax error arrives with a byte range already
//!   ([`PresetError::span`](rlx_core::preset::PresetError::span)), and that is the
//!   only arm that does.
//! - An expression error names its parameter
//!   ([`PresetError::param`](rlx_core::preset::PresetError::param)), so the
//!   key is looked up in a second, span-carrying parse of the same source
//!   ([`toml::de::DeTable::parse`]) and the diagnostic lands on that key.
//! - A structural-config error and every entry of [`Preset::warnings`] are
//!   strings with nothing positional in them, so they are reported at the file
//!   level — line 1, column 1.
//!
//! `toml` is already a dependency of this crate for `config.toml`, so the
//! spanned parse costs no new name in the graph (NFR section 4).
//!
//! ## Nothing here writes
//!
//! There is no `--fix` and no formatter: the checker reads a file and prints.
//! ADR-0190 records the measurement that decided it.

use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use rlx_core::preset::{Preset, PresetError};
use toml::de::{DeTable, DeValue};

/// How much a diagnostic costs the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The preset does not load. Exit non-zero.
    Error,
    /// The preset loads and something about it is probably not what was meant.
    /// Exit zero unless `--strict`.
    Warning,
}

impl Severity {
    /// The word the diagnostic line carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The rule every diagnostic the loader produced is filed under.
///
/// One id for all of them on purpose: the engine is a single authority, and
/// splitting its verdicts into rules would invite suppressing one of them.
pub const RULE_ENGINE: &str = "engine";

/// One thing to say about one file.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Which rule produced it — [`RULE_ENGINE`] for everything the loader said.
    pub rule: &'static str,
    /// The byte range in the source this points at. `None` is the file level,
    /// rendered as `1:1`.
    pub span: Option<Range<usize>>,
    pub message: String,
}

impl Diagnostic {
    /// An error at a position (or at the file level when `span` is `None`).
    pub fn error(rule: &'static str, span: Option<Range<usize>>, message: String) -> Self {
        Self {
            severity: Severity::Error,
            rule,
            span,
            message,
        }
    }

    /// A warning at a position (or at the file level when `span` is `None`).
    pub fn warning(rule: &'static str, span: Option<Range<usize>>, message: String) -> Self {
        Self {
            severity: Severity::Warning,
            rule,
            span,
            message,
        }
    }

    /// The diagnostic as one line: `path:line:col: severity[rule]: message`.
    ///
    /// `path` is rendered as it was given on the command line rather than
    /// canonicalized, so an editor's problem matcher resolves it against the
    /// same working directory the operator typed it in.
    pub fn render(&self, path: &Path, src: &str) -> String {
        let (line, col) = match &self.span {
            Some(span) => line_col(src, span.start),
            None => (1, 1),
        };
        format!(
            "{}:{line}:{col}: {}[{}]: {}",
            path.display(),
            self.severity,
            self.rule,
            self.message
        )
    }
}

/// The 1-based line and column of byte offset `at`.
///
/// The column counts **Unicode scalar values**, not bytes: an editor's gutter
/// counts characters, and a preset comment carrying an em dash would otherwise
/// push every column on its line. An offset past the end of `src` clamps to the
/// last position rather than panicking — a span is the parser's claim about the
/// source, and a checker must not be the thing that crashes on a bad one.
fn line_col(src: &str, at: usize) -> (usize, usize) {
    let at = at.min(src.len());
    let before = &src[..find_char_boundary(src, at)];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let col = match before.rfind('\n') {
        Some(nl) => before[nl + 1..].chars().count() + 1,
        None => before.chars().count() + 1,
    };
    (line, col)
}

/// The largest char boundary at or below `at`.
///
/// `DeTable`'s spans are char-aligned, so this only ever returns `at` on real
/// input; it exists so a slice of `src` cannot panic if that ever stops holding.
fn find_char_boundary(src: &str, at: usize) -> usize {
    let mut at = at;
    while at > 0 && !src.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Every diagnostic the engine has about `src`, read as the preset at `path`.
///
/// `path` is carried for rendering and for the rules that judge a filename; the
/// loader itself never sees it.
pub fn check(path: &Path, src: &str) -> Vec<Diagnostic> {
    let _ = path;
    engine_diagnostics(src)
}

/// The loader's verdict, positioned.
///
/// Separate from [`check`] so the house-style rules can be added beside it
/// rather than inside it: this function is the one place the engine is asked,
/// and it must stay exactly as forgiving as the engine is.
fn engine_diagnostics(src: &str) -> Vec<Diagnostic> {
    match Preset::from_toml_str(src) {
        // A load failure is one diagnostic: the loader stops at the first
        // problem, so a second one here would be invented rather than reported.
        Err(err) => {
            let span = err
                .span()
                .or_else(|| err.param().and_then(|param| locate_param(src, param)));
            vec![Diagnostic::error(RULE_ENGINE, span, message_of(&err))]
        }
        Ok(preset) => preset
            .warnings
            .into_iter()
            // File level: a warning is a `String` with nothing positional in
            // it. Giving `Preset::warnings` structure is a core change with the
            // studio as a second consumer, so the editor schema is what places
            // the common unknown-key case for now (ADR-0190).
            .map(|message| Diagnostic::warning(RULE_ENGINE, None, message))
            .collect(),
    }
}

/// One line of prose for a load failure.
///
/// The `Toml` arm is read through [`toml::de::Error::message`] rather than
/// through its `Display`, which renders a multi-line snippet with a caret. That
/// snippet says `TOML parse error at line 6, column 8` and then draws the line —
/// a position this diagnostic already carries in its own `line:col`, so printing
/// it costs three lines to repeat one fact. Every other arm is already a
/// sentence.
///
/// The whitespace collapse is the invariant behind the format rather than a
/// cosmetic: one diagnostic is one line, and a message carrying a newline would
/// silently produce two that no `file:line:col` matcher can read.
fn message_of(err: &PresetError) -> String {
    let text = match err {
        PresetError::Toml(inner) => inner.message().to_owned(),
        other => other.to_string(),
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The span of the key an expression error names, in a spanned re-parse of the
/// same source.
///
/// The loader labels a failing expression with the surface it came from, and
/// those labels are what this peels back into a key path:
///
/// | `PresetError::param()`      | key path                 |
/// |----------------------------|--------------------------|
/// | `glow`                     | `params.glow`            |
/// | `[layer] glow`             | `layer.params.glow`      |
/// | `[layer] mix`              | `layer.mix`              |
/// | `[per_vertex] x`           | `per_vertex.x`           |
/// | `[layer] [per_vertex] x`   | `layer.per_vertex.x`     |
/// | `[latch] pulse.arm`        | `latch.pulse.arm`        |
///
/// `None` when the path is not in the document — which happens when the loader
/// grows a label this does not know, and then the diagnostic falls back to the
/// file level rather than to a guess at some other key of the same name.
fn locate_param(src: &str, param: &str) -> Option<Range<usize>> {
    let doc = DeTable::parse(src).ok()?;
    let path = key_path(param)?;
    key_span(doc.get_ref(), &path)
}

/// [`locate_param`]'s table, as a path of key names.
fn key_path(param: &str) -> Option<Vec<String>> {
    let mut path: Vec<String> = Vec::new();
    let mut rest = param;

    // `[latch]` is labelled without a surface prefix, so it is read first and
    // is the whole path when it matches.
    if let Some(entry) = rest.strip_prefix("[latch] ") {
        let (name, key) = entry.split_once('.')?;
        return Some(vec!["latch".into(), name.into(), key.into()]);
    }

    if let Some(inner) = rest.strip_prefix("[layer] ") {
        path.push("layer".into());
        rest = inner;
    }

    if let Some(name) = rest.strip_prefix("[per_vertex] ") {
        path.push("per_vertex".into());
        path.push(name.into());
        return Some(path);
    }

    // `[layer] mix` is the layer's own key rather than one of its bindings, and
    // it is the only such label: every other layer diagnostic names a binding.
    if path.len() == 1 && rest == "mix" {
        path.push("mix".into());
        return Some(path);
    }

    // A label carrying a bracketed table this function does not know would
    // otherwise be looked up as a parameter spelled `[something] name`, which no
    // document has. Reporting at the file level says less and claims nothing
    // false.
    if rest.contains('[') {
        return None;
    }

    path.push("params".into());
    path.push(rest.into());
    Some(path)
}

/// The span of the **key** at `path`, walking tables from `table`.
///
/// The key rather than its value: a diagnostic about `glow = "bass * "` belongs
/// on `glow`, which is where an author looks and where an editor puts the
/// squiggle.
fn key_span(table: &DeTable<'_>, path: &[String]) -> Option<Range<usize>> {
    let (last, parents) = path.split_last()?;
    let mut current = table;
    // Held across the loop so the borrow of the value that owns the next table
    // outlives the step that produced it.
    let mut held;
    for name in parents {
        let value = lookup(current, name)?.1;
        held = value;
        current = held.get_ref().as_table()?;
    }
    Some(lookup(current, last)?.0)
}

/// The span of key `name` in `table`, with the value beside it.
///
/// A linear walk rather than `Map::get`, because both halves are wanted and the
/// spanned key is what carries the position.
fn lookup<'t, 'i>(
    table: &'t DeTable<'i>,
    name: &str,
) -> Option<(Range<usize>, &'t toml::Spanned<DeValue<'i>>)> {
    table
        .iter()
        .find(|(key, _)| key.get_ref().as_ref() == name)
        .map(|(key, value)| (key.span(), value))
}

/// What a checked directory or file amounted to, for the summary line and the
/// exit code.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub files: usize,
    pub errors: usize,
    pub warnings: usize,
}

impl Tally {
    /// Count one file's diagnostics.
    pub fn add(&mut self, diagnostics: &[Diagnostic]) {
        self.files += 1;
        for diagnostic in diagnostics {
            match diagnostic.severity {
                Severity::Error => self.errors += 1,
                Severity::Warning => self.warnings += 1,
            }
        }
    }

    /// The process exit code: `0` clean, `1` on an error — and on a warning too
    /// when `strict`.
    pub fn exit_code(self, strict: bool) -> i32 {
        let failed = self.errors > 0 || (strict && self.warnings > 0);
        i32::from(failed)
    }

    /// The one-line summary the run writes to standard error.
    ///
    /// Standard error rather than out, so a caller piping the diagnostics into a
    /// problem matcher gets diagnostics alone on the stream it parses.
    pub fn summary(self) -> String {
        let files = plural(self.files, "file");
        let errors = plural(self.errors, "error");
        let warnings = plural(self.warnings, "warning");
        format!("checked {files}: {errors}, {warnings}")
    }
}

/// `1 file` / `2 files` — English, not a count with a bare `(s)`.
fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Why a `--check` run could not happen at all, as distinct from a preset that
/// failed it.
///
/// Reported by the caller with exit `2`, the code this binary already uses for
/// an argument list that is wrong in shape — a missing path is the operator's
/// spelling, not the preset's content.
#[derive(Debug)]
pub enum CheckFailure {
    /// The path names nothing, or could not be read.
    Unreadable { path: PathBuf, error: String },
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckFailure::Unreadable { path, error } => {
                write!(f, "--check {}: {error}", path.display())
            }
        }
    }
}

/// The `*.toml` files `--check <path>` reads, in a stable order.
///
/// A file is itself, whatever its extension — someone naming a file means that
/// file. A directory yields its `*.toml` entries **non-recursively**, the
/// convention `core/build.rs` already follows for the embedded set so that a
/// subdirectory is held back by construction (ADR-0022).
pub fn targets(path: &Path) -> Result<Vec<PathBuf>, CheckFailure> {
    let metadata = std::fs::metadata(path).map_err(|error| CheckFailure::Unreadable {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    if !metadata.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let entries = std::fs::read_dir(path).map_err(|error| CheckFailure::Unreadable {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    // `read_dir` order is the filesystem's, and a diagnostic list that reorders
    // itself between runs cannot be diffed.
    files.sort();
    // A directory holding no presets is not an error: `presets/pending/` is
    // empty whenever nothing is waiting on an engine gap, and the gate reads it
    // every run. The summary line reports the count, so a run over nothing says
    // `checked 0 files` rather than passing silently.
    Ok(files)
}

/// Check every file `path` names, printing one line per diagnostic to standard
/// output and the summary to standard error.
///
/// Returns the process exit code. Nothing here opens a GPU adapter, an audio
/// device or a window — the whole point is that an author's "does this compile"
/// loop costs one process start (ADR-0190).
pub fn run(path: &Path, strict: bool) -> Result<i32, CheckFailure> {
    let mut tally = Tally::default();
    for file in targets(path)? {
        let src = std::fs::read_to_string(&file).map_err(|error| CheckFailure::Unreadable {
            path: file.clone(),
            error: error.to_string(),
        })?;
        let diagnostics = check(&file, &src);
        for diagnostic in &diagnostics {
            println!("{}", diagnostic.render(&file, &src));
        }
        tally.add(&diagnostics);
    }
    eprintln!("{}", tally.summary());
    Ok(tally.exit_code(strict))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_one_based_and_counts_characters() {
        let src = "system = \"x\"\n# a — dash\nglow = \"1\"\n";
        assert_eq!(line_col(src, 0), (1, 1));
        // The byte offset of `glow`, which follows a line carrying a 3-byte em
        // dash: a byte-counted column would be right only by accident here.
        let at = src.find("glow").expect("the fixture has a glow key");
        assert_eq!(line_col(src, at), (3, 1));
        // The offset of the em dash itself, four characters into line 2.
        let dash = src.find('—').expect("the fixture has an em dash");
        assert_eq!(line_col(src, dash), (2, 5));
    }

    #[test]
    fn an_offset_past_the_end_clamps() {
        let src = "system = \"x\"\n";
        assert_eq!(line_col(src, 9_999), (2, 1));
    }

    #[test]
    fn the_loader_labels_peel_into_key_paths() {
        let path = |p: &str| key_path(p).map(|v| v.join("."));
        assert_eq!(path("glow").as_deref(), Some("params.glow"));
        assert_eq!(path("[layer] glow").as_deref(), Some("layer.params.glow"));
        assert_eq!(path("[layer] mix").as_deref(), Some("layer.mix"));
        assert_eq!(path("[per_vertex] x").as_deref(), Some("per_vertex.x"));
        assert_eq!(
            path("[layer] [per_vertex] x").as_deref(),
            Some("layer.per_vertex.x")
        );
        assert_eq!(
            path("[latch] pulse.arm").as_deref(),
            Some("latch.pulse.arm")
        );
        // A label this function does not know reports at the file level rather
        // than resolving to a parameter nothing calls that.
        assert_eq!(path("[future] thing"), None);
    }

    #[test]
    fn a_binding_key_is_located_in_the_document() {
        let src = "system = \"fragment_field\"\n\n[params]\nglow = \"1\"\nzoom = \"2\"\n";
        let span = locate_param(src, "zoom").expect("zoom is in the document");
        assert_eq!(&src[span.clone()], "zoom");
        assert_eq!(line_col(src, span.start), (5, 1));
    }

    #[test]
    fn a_latch_key_is_located_through_its_entry() {
        let src = "system = \"fragment_field\"\n\n[latch.pulse]\narm = \"beat\"\nfire = \"1\"\n";
        let span = locate_param(src, "[latch] pulse.fire").expect("the latch key is there");
        assert_eq!(&src[span.clone()], "fire");
        assert_eq!(line_col(src, span.start), (5, 1));
    }

    #[test]
    fn a_key_the_document_does_not_have_is_no_position() {
        let src = "system = \"fragment_field\"\n\n[params]\nglow = \"1\"\n";
        assert!(locate_param(src, "nothing_named_this").is_none());
    }

    #[test]
    fn the_exit_code_is_warning_sensitive_only_under_strict() {
        let clean = Tally {
            files: 1,
            errors: 0,
            warnings: 0,
        };
        let warned = Tally {
            files: 1,
            errors: 0,
            warnings: 1,
        };
        let failed = Tally {
            files: 1,
            errors: 1,
            warnings: 0,
        };
        assert_eq!(clean.exit_code(false), 0);
        assert_eq!(clean.exit_code(true), 0);
        assert_eq!(warned.exit_code(false), 0);
        assert_eq!(warned.exit_code(true), 1);
        assert_eq!(failed.exit_code(false), 1);
        assert_eq!(failed.exit_code(true), 1);
    }

    #[test]
    fn the_summary_counts_in_english() {
        let one = Tally {
            files: 1,
            errors: 1,
            warnings: 0,
        };
        assert_eq!(one.summary(), "checked 1 file: 1 error, 0 warnings");
        let many = Tally {
            files: 12,
            errors: 0,
            warnings: 3,
        };
        assert_eq!(many.summary(), "checked 12 files: 0 errors, 3 warnings");
    }
}
