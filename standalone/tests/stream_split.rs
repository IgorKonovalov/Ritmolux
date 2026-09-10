//! The two streams stay separable by their first byte (ADR-0176, Plan 0158
//! Phase 3).
//!
//! A parent reading this player's standard error routes each line on its first
//! character: `{` is a structured event, anything else is a human diagnostic. It
//! is a contract with no framing behind it, which is what makes it cheap — and
//! also what makes it breakable by one `eprintln!` written without thinking
//! about it.
//!
//! So this reads the standalone's own **source** and holds every diagnostic
//! macro to the rule. A source scan rather than a runtime check, for the reason
//! the roster and header greps elsewhere in this suite are: a run exercises the
//! lines that happened to be reached, and the property is about all of them.
//!
//! The events side of the same contract is asserted next to the writer, in
//! `standalone/src/events.rs`.

use std::path::{Path, PathBuf};

/// A leading placeholder whose value has been checked and cannot begin with `{`.
///
/// `eprintln!("{msg}")` begins with a brace in the *source* and never in the
/// *output*. Two classes qualify and both are here: an operator-readable
/// sentence the shell assembled elsewhere (`msg`, `message`, `err`, and the
/// positional `{}` that stands for a `Display`), and a compile-time constant
/// whose value is fixed in this crate (`PRESET_DIR_ENV` is `"RLX_PRESET_DIR"`).
///
/// This list is the whole allowlist, and a new leading placeholder fails the
/// test until somebody has looked at what it interpolates — which is the point,
/// since the alternative is a rule nobody is reminded of.
const MESSAGE_PLACEHOLDERS: [&str; 5] = ["{}", "{msg}", "{message}", "{err}", "{PRESET_DIR_ENV}"];

/// Every `.rs` file under `standalone/src/`.
fn sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The format-string literal that opens `macro!(` at `rest`, or `None` when the
/// first argument is not a literal.
///
/// Deliberately simple: it reads to the closing quote, honouring `\"`. Every
/// call site in this crate opens with a plain literal, and one that did not would
/// return `None` and be reported by the caller rather than silently skipped.
fn opening_literal(rest: &str) -> Option<&str> {
    let body = rest.trim_start();
    let inner = body.strip_prefix('"')?;
    let mut escaped = false;
    for (i, ch) in inner.char_indices() {
        match ch {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return inner.get(..i),
            _ => {}
        }
    }
    None
}

/// Every use of `macro` in `source`, as its format literal.
///
/// The name is matched on an identifier boundary, which is load-bearing rather
/// than tidy: `eprintln!(` **contains** `println!(`, so a search without it
/// would report every diagnostic in the crate as a write to standard output.
fn format_literals(source: &str, macro_name: &str) -> Vec<String> {
    let needle = format!("{macro_name}!(");
    let mut out = Vec::new();
    let mut at = 0usize;
    while let Some(found) = source.get(at..).and_then(|s| s.find(&needle)) {
        let opens = at + found;
        let start = opens + needle.len();
        let preceded_by_identifier = source
            .get(..opens)
            .and_then(|before| before.chars().next_back())
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        if !preceded_by_identifier
            && let Some(literal) = source.get(start..).and_then(opening_literal)
        {
            out.push(literal.to_owned());
        }
        at = start;
    }
    out
}

/// Every file under `standalone/src/` that writes to standard output at all,
/// paired with why it is allowed to.
///
/// **Standard output is the frame pipe's** while a sink is open (ADR-0176), and
/// the pipe writes bytes rather than lines — so nothing in this list may be
/// reachable from a running `--stream`. Each entry below either exits the
/// process before a sink could exist, or belongs to a different binary
/// entirely. A new file writing to stdout fails the test until somebody has
/// checked which of those it is.
const STDOUT_WRITERS: [(&str, &str); 5] = [
    (
        "capture_win.rs",
        "--list-devices prints the roster and exits",
    ),
    ("cli.rs", "--help prints the flag roster and exits"),
    ("run.rs", "--schema prints the document and exits"),
    (
        "horizon.rs",
        "the `shot` CLI, a separate binary that opens no sink",
    ),
    (
        "report.rs",
        "the `shot` CLI, a separate binary that opens no sink",
    ),
];

/// Standard output is written only by the places that exit before a sink could
/// be open, and by the sink itself.
#[test]
fn nothing_writes_prose_to_standard_output_while_a_sink_could_be_open() {
    let allowed: Vec<&str> = STDOUT_WRITERS.iter().map(|(file, _)| *file).collect();
    let mut findings = Vec::new();
    for path in sources() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let writes =
            format_literals(&source, "println").len() + format_literals(&source, "print").len();
        if writes == 0 {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !allowed.contains(&name.as_str()) {
            findings.push(format!(
                "  {}: {writes} write(s) to standard output",
                path.display()
            ));
        }
    }
    assert!(
        findings.is_empty(),
        "{} file(s) write to standard output and are not in STDOUT_WRITERS:\n{}\n\
         Standard output carries the frame pipe while --stream --sink stdout is \
         open, so prose on it corrupts the stream. Either move the line to \
         stderr, or add the file with the reason it can never be reached from a \
         running stream.",
        findings.len(),
        findings.join("\n"),
    );

    // ...and the allowlist does not outlive what it describes.
    let mut unused: Vec<&str> = Vec::new();
    for (file, _) in STDOUT_WRITERS {
        let found = sources().iter().any(|path| {
            path.file_name().is_some_and(|n| n == file)
                && std::fs::read_to_string(path).is_ok_and(|source| {
                    !format_literals(&source, "println").is_empty()
                        || !format_literals(&source, "print").is_empty()
                })
        });
        if !found {
            unused.push(file);
        }
    }
    assert!(
        unused.is_empty(),
        "these files are allowlisted for standard output and no longer write \
         to it: {unused:?}"
    );
}

/// Whether `literal` can render a line that begins with `{`.
fn can_begin_with_a_brace(literal: &str) -> bool {
    if !literal.starts_with('{') {
        return false;
    }
    // `{{` is an escaped brace: the line really does begin with one.
    if literal.starts_with("{{") {
        return true;
    }
    !MESSAGE_PLACEHOLDERS
        .iter()
        .any(|allowed| literal.starts_with(allowed))
}

/// No human diagnostic can begin with `{`, so a parent's split on the first byte
/// cannot mistake one for an event.
#[test]
fn no_human_diagnostic_line_can_begin_with_a_brace() {
    let mut findings = Vec::new();
    for path in sources() {
        // The writer's own job is to produce lines that begin with `{`.
        if path.file_name().is_some_and(|name| name == "events.rs") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for literal in format_literals(&source, "eprintln") {
            if can_begin_with_a_brace(&literal) {
                findings.push(format!("  {}: eprintln!(\"{literal}\"…)", path.display()));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "{} diagnostic(s) can render a line beginning with `{{`, which a parent \
         reading the event stream would route as an event:\n{}\n\
         Give the line a prefix, or — if it interpolates a message the shell \
         built and cannot begin with a brace — add its spelling to \
         MESSAGE_PLACEHOLDERS after checking that.",
        findings.len(),
        findings.join("\n"),
    );
}

/// The scan finds something, so a green run above is evidence rather than an
/// empty walk.
#[test]
fn the_scan_reaches_the_diagnostics_it_is_supposed_to_hold() {
    let total: usize = sources()
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|source| format_literals(&source, "eprintln").len())
        .sum();
    assert!(
        total > 50,
        "the scan found only {total} eprintln! call sites in standalone/src, \
         which is too few to be a walk of this crate"
    );
}

/// The allowlist is not a way to admit anything: each entry is a leading
/// placeholder, never a literal brace.
#[test]
fn the_allowlist_admits_only_placeholders() {
    for allowed in MESSAGE_PLACEHOLDERS {
        assert!(
            allowed.starts_with('{') && allowed.ends_with('}') && !allowed.starts_with("{{"),
            "`{allowed}` is not a leading placeholder"
        );
        assert!(
            !can_begin_with_a_brace(allowed),
            "`{allowed}` is allowlisted but still reported"
        );
    }
    // The rule it exists to enforce, on the shapes it must still catch.
    for refused in ["{{", "{{\"v\":1}}", "{ not a placeholder"] {
        assert!(
            can_begin_with_a_brace(refused),
            "`{refused}` renders a leading brace and was not caught"
        );
    }
}

// ---------------------------------------------------------------------------
// The same contract from outside the process. `--preset` with an unknown name
// is the one refusal that happens *after* the player has greeted, which makes
// it the only place a spawned run can be watched saying hello without a window.
// ---------------------------------------------------------------------------

/// Run `ritmolux` with `args` and return its stdout and stderr.
fn run(args: &[&str]) -> (String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .args(args)
        .output()
        .expect("spawn the built binary");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A name no roster holds, so the refusal path is taken whatever is installed.
const NO_SUCH_PRESET: &str = "__no_such_preset_0158__";

/// `hello` is the first **event**, and it carries the three facts a studio needs
/// before it can drive the player.
///
/// First among the events rather than first among all the lines, and the
/// distinction is forced rather than chosen: `hello` reports the control port
/// actually bound, that port comes from the operator config, the config path is
/// resolved under the per-user directory, and that directory may still need
/// migrating — so the migration's own notice, and the tier and input lines a
/// configured rig prints, can precede the greeting. A parent splits on the first
/// byte and waits for `hello`, which those lines do not disturb.
#[test]
fn hello_is_the_first_event_and_carries_the_version_schema_and_port() {
    let (_, stderr) = run(&["--events", "--preset", NO_SUCH_PRESET]);
    let events: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with('{'))
        .collect();
    let first = events
        .first()
        .unwrap_or_else(|| panic!("no event line at all on stderr:\n{stderr}"));
    assert!(
        first.contains("\"ev\":\"hello\""),
        "the first event is not hello: {first}"
    );
    assert!(
        first.contains(&format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"))),
        "hello does not carry this build's version: {first}"
    );
    assert!(
        first.contains("\"schema\":\""),
        "hello does not carry a schema hash: {first}"
    );
    // No `--control`, so no listener was opened and the field says so rather
    // than naming a port nothing is bound to.
    assert!(
        first.contains("\"control\":null"),
        "hello named a control port on a run that opened no socket: {first}"
    );
}

/// `hello` reports the port **actually bound**, which a port of 0 makes
/// different from the one requested.
#[test]
fn hello_reports_the_port_actually_bound() {
    let (_, stderr) = run(&[
        "--events",
        "--control",
        "127.0.0.1:0",
        "--preset",
        NO_SUCH_PRESET,
    ]);
    let first = stderr
        .lines()
        .find(|line| line.starts_with('{'))
        .unwrap_or_else(|| panic!("no event line at all on stderr:\n{stderr}"));
    assert!(
        first.contains("\"control\":\"127.0.0.1:"),
        "hello does not name a loopback control port: {first}"
    );
    assert!(
        !first.contains("\"control\":\"127.0.0.1:0\""),
        "hello echoed the requested port rather than the one the stack chose, \
         so a studio asking for an ephemeral port cannot find it: {first}"
    );
}

/// Without `--events`, standard error carries no event line at all.
///
/// The checkable form of "nothing on standard error changes": the human
/// diagnostics are compared against the same run's own output below, and this is
/// the assertion that the feature adds nothing when it was not asked for.
#[test]
fn without_the_flag_standard_error_carries_no_event_line() {
    let (_, stderr) = run(&["--preset", NO_SUCH_PRESET]);
    let events: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with('{'))
        .collect();
    assert!(
        events.is_empty(),
        "a run that did not ask for events produced {}:\n{}",
        events.len(),
        events.join("\n")
    );
    assert!(
        !stderr.is_empty(),
        "the run produced no stderr at all, so this assertion is vacuous"
    );
}

/// The human diagnostics are the same with the flag and without it.
///
/// Byte identity after the event lines are removed: `--events` **adds** a
/// stream, and an operator reading the console must not be able to tell it was
/// passed.
#[test]
fn the_flag_adds_lines_and_changes_none() {
    let (_, plain) = run(&["--preset", NO_SUCH_PRESET]);
    let (_, evented) = run(&["--events", "--preset", NO_SUCH_PRESET]);

    let human: Vec<&str> = evented
        .lines()
        .filter(|line| !line.starts_with('{'))
        .collect();
    let baseline: Vec<&str> = plain.lines().collect();
    assert_eq!(
        human, baseline,
        "the human diagnostics differ between a run with --events and one \
         without, so the flag is not purely additive"
    );
    assert!(
        evented.lines().any(|line| line.starts_with('{')),
        "the evented run produced no event lines, so this comparison is vacuous"
    );
}
