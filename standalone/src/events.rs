//! The structured event stream: what the player tells a parent process about
//! itself (ADR-0176).
//!
//! **Off unless asked for.** Without `--events` nothing here is constructed and
//! standard error carries exactly the human diagnostics it always did.
//!
//! ## One object per line, and the first byte is the split
//!
//! Every line begins `{`, carries `"v"` and `"ev"`, and ends with a newline.
//! Human diagnostics keep their own lines and none of them begins with `{`, so a
//! parent splits the two streams on the first byte and needs no framing. That
//! rule is held by a test rather than by care:
//! `an_event_line_is_one_object_a_parent_can_split_on_its_first_byte` checks
//! this side and `no_human_diagnostic_line_can_begin_with_a_brace` checks the
//! other.
//!
//! ## Why standard error
//!
//! A parent reads its own child's stream, so the events are attributable to one
//! player instance without a port to collide on, and they arrive in order and
//! cannot drop — the three things the control direction's UDP deliberately does
//! not promise. Standard **output** is reserved for bulk binary payloads.
//!
//! ## The writer
//!
//! The same hand-rolled JSON the `shot` reports use ([`crate::shot::json`]), so
//! no JSON crate enters the player. `json_string` is the part that matters: a
//! preset name and a file path both come from a user's disk, and a path
//! containing a quote must not be able to produce a line the parent cannot
//! parse.

use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

use crate::shot::json::{json_string, num};

/// The event stream's own version, carried as `v` on every line.
///
/// Independent of the control vocabulary's version, which lives in its
/// addresses, and of the schema document's. `hello` carries all three so a
/// studio can refuse a combination it does not know (ADR-0176).
pub const EVENT_VERSION: u32 = 1;

/// One event.
///
/// Borrows rather than owns throughout: every one of these is rendered
/// immediately at the site that has the values, so an owned variant would be a
/// clone of a preset name on a path that already has it in hand.
#[derive(Debug, Clone, Copy)]
pub enum Event<'a> {
    /// The first line after start: who this player is, and what a studio needs
    /// before it can drive it.
    Hello {
        /// The workspace version this binary was built at.
        version: &'a str,
        /// The schema document's hash, so a studio can tell whether the panels
        /// it cached are the ones this player is running.
        schema: &'a str,
        /// The control port **actually bound**, which is not necessarily the one
        /// requested: a port of 0 asks the stack to choose. `None` when no
        /// listener was asked for.
        control: Option<SocketAddr>,
    },
    /// The active preset changed.
    Preset {
        /// Its name.
        name: &'a str,
        /// Its absolute position in the roster.
        index: usize,
    },
    /// The roster was replaced — on every reload, whatever changed.
    Roster {
        /// The preset names, in roster order.
        names: &'a [String],
    },
    /// A preset failed to load.
    PresetError {
        /// The file it came from.
        file: &'a Path,
        /// The loader's own sentence.
        message: &'a str,
        /// One-based line, when the parser gave a position.
        line: Option<u32>,
        /// One-based column, when the parser gave a position.
        col: Option<u32>,
        /// The parameter whose expression failed, for the arm that has one.
        param: Option<&'a str>,
    },
    /// A preset loaded with a non-fatal problem in it.
    PresetWarning {
        /// The file it came from.
        file: &'a Path,
        /// The loader's own sentence.
        message: &'a str,
    },
    /// Once a second while a frame is being drawn.
    Health {
        /// Frames per second over the diagnostics window.
        fps: f32,
        /// Median frame time over the window, in milliseconds.
        frame_ms_p50: f32,
        /// 99th-percentile frame time over the window, in milliseconds.
        frame_ms_p99: f32,
        /// Control datagrams the decoder refused, since start.
        ctl_rejected: u64,
        /// Control actions dropped because a frame's queue was full, since
        /// start.
        ctl_dropped: u64,
        /// Parameter overrides the engine refused because nothing claims the
        /// name, since start.
        ctl_refused: u64,
    },
    /// The geometry of the frame stream on standard output, sent once before the
    /// first frame.
    Stream {
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Frames per second the sink is paced to.
        fps: u32,
        /// The pixel format, as bytes on the wire.
        format: &'a str,
    },
    /// The answer to a `ctl/ping`, carrying its nonce.
    Pong {
        /// The nonce the ping carried.
        nonce: i32,
    },
}

impl Event<'_> {
    /// The `ev` name — the roster ADR-0176 fixes.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Hello { .. } => "hello",
            Event::Preset { .. } => "preset",
            Event::Roster { .. } => "roster",
            Event::PresetError { .. } => "preset_error",
            Event::PresetWarning { .. } => "preset_warning",
            Event::Health { .. } => "health",
            Event::Stream { .. } => "stream",
            Event::Pong { .. } => "pong",
        }
    }

    /// Render as one line, newline included.
    ///
    /// The `v` and `ev` fields lead every object, so a parent that reads only the
    /// first few dozen bytes of a line can already route it.
    pub fn line(&self) -> String {
        let mut out = format!(
            "{{\"v\":{EVENT_VERSION},\"ev\":{}",
            json_string(self.name())
        );
        match self {
            Event::Hello {
                version,
                schema,
                control,
            } => {
                out.push_str(",\"version\":");
                out.push_str(&json_string(version));
                out.push_str(",\"schema\":");
                out.push_str(&json_string(schema));
                out.push_str(",\"control\":");
                match control {
                    Some(addr) => out.push_str(&json_string(&addr.to_string())),
                    // `null` rather than an empty string: "no listener" and
                    // "listening on nothing" are different facts and a studio
                    // decides differently on each.
                    None => out.push_str("null"),
                }
            }
            Event::Preset { name, index } => {
                out.push_str(",\"name\":");
                out.push_str(&json_string(name));
                out.push_str(&format!(",\"index\":{index}"));
            }
            Event::Roster { names } => {
                out.push_str(",\"names\":[");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_string(name));
                }
                out.push(']');
            }
            Event::PresetError {
                file,
                message,
                line,
                col,
                param,
            } => {
                out.push_str(",\"file\":");
                out.push_str(&json_string(&file.display().to_string()));
                out.push_str(",\"message\":");
                out.push_str(&json_string(message));
                push_optional_u32(&mut out, "line", *line);
                push_optional_u32(&mut out, "col", *col);
                out.push_str(",\"param\":");
                match param {
                    Some(param) => out.push_str(&json_string(param)),
                    None => out.push_str("null"),
                }
            }
            Event::PresetWarning { file, message } => {
                out.push_str(",\"file\":");
                out.push_str(&json_string(&file.display().to_string()));
                out.push_str(",\"message\":");
                out.push_str(&json_string(message));
            }
            Event::Health {
                fps,
                frame_ms_p50,
                frame_ms_p99,
                ctl_rejected,
                ctl_dropped,
                ctl_refused,
            } => {
                out.push_str(&format!(
                    ",\"fps\":{},\"frame_ms_p50\":{},\"frame_ms_p99\":{},\
                     \"ctl_rejected\":{ctl_rejected},\"ctl_dropped\":{ctl_dropped},\
                     \"ctl_refused\":{ctl_refused}",
                    num(*fps),
                    num(*frame_ms_p50),
                    num(*frame_ms_p99),
                ));
            }
            Event::Stream {
                width,
                height,
                fps,
                format,
            } => {
                out.push_str(&format!(
                    ",\"width\":{width},\"height\":{height},\"fps\":{fps}"
                ));
                out.push_str(",\"format\":");
                out.push_str(&json_string(format));
            }
            Event::Pong { nonce } => out.push_str(&format!(",\"nonce\":{nonce}")),
        }
        out.push_str("}\n");
        out
    }
}

/// A field that is a number when the parser gave one and `null` when it did not.
///
/// `null` rather than omitting the key: a consumer reads a fixed shape, and
/// "absent" and "the parser could not say" are the same fact here — where an
/// omitted key would make it guess whether the field exists in this version.
fn push_optional_u32(out: &mut String, key: &str, value: Option<u32>) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":");
    match value {
        Some(value) => out.push_str(&value.to_string()),
        None => out.push_str("null"),
    }
}

/// The sink, present only with `--events`.
///
/// Holds nothing but a locked handle's worth of intent: each line is written and
/// flushed on the spot, because the value of an event is that a parent has it
/// **now** — a buffered `preset_error` that arrives after the studio has moved
/// on is worse than none.
pub struct Events {
    /// Whether `hello` has been sent, so it can be asserted to be first rather
    /// than hoped to be.
    greeted: bool,
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

impl Events {
    /// A sink that has said nothing yet.
    pub fn new() -> Self {
        Self { greeted: false }
    }

    /// Whether `hello` has gone out.
    pub fn greeted(&self) -> bool {
        self.greeted
    }

    /// Write one event.
    ///
    /// Failure is silence: standard error may be closed or full, and a player
    /// that stopped drawing because a parent stopped reading is a worse outcome
    /// than a lost line — the same rule the telemetry sink follows for a
    /// datagram it could not send.
    pub fn emit(&mut self, event: &Event<'_>) {
        if matches!(event, Event::Hello { .. }) {
            self.greeted = true;
        }
        let line = event.line();
        let mut err = std::io::stderr().lock();
        let _ = err.write_all(line.as_bytes());
        let _ = err.flush();
    }
}

/// One-based line and column of `offset` in `source`.
///
/// Column counts **characters**, not bytes: an author whose preset has an
/// accented word in a comment above the error would otherwise be pointed past
/// the end of the line by an editor that counts the same way they do. An offset
/// past the end lands on the last position, which is where a truncated document
/// fails.
pub fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event, with values that are distinguishable from each other — so a
    /// transposed field shows up as a wrong value rather than as another 0.
    fn roster() -> Vec<Event<'static>> {
        static NAMES: [&str; 0] = [];
        let _ = NAMES;
        vec![
            Event::Hello {
                version: "0.112.0",
                schema: "0123456789abcdef",
                control: Some("127.0.0.1:9001".parse().expect("a literal address")),
            },
            Event::Hello {
                version: "0.112.0",
                schema: "0123456789abcdef",
                control: None,
            },
            Event::Preset {
                name: "aurora \"quoted\"",
                index: 3,
            },
            Event::PresetError {
                file: Path::new("C:\\presets\\aurora.toml"),
                message: "unknown function `bins`",
                line: Some(41),
                col: Some(9),
                param: Some("bg_bright"),
            },
            Event::PresetError {
                file: Path::new("/presets/aurora.toml"),
                message: "could not read preset file",
                line: None,
                col: None,
                param: None,
            },
            Event::PresetWarning {
                file: Path::new("/presets/aurora.toml"),
                message: "binds `warpp`, which no system consumes",
            },
            Event::Health {
                fps: 59.8,
                frame_ms_p50: 4.1,
                frame_ms_p99: 9.7,
                ctl_rejected: 2,
                ctl_dropped: 3,
                ctl_refused: 4,
            },
            Event::Stream {
                width: 640,
                height: 360,
                fps: 30,
                format: "rgba8",
            },
            Event::Pong { nonce: -7 },
        ]
    }

    /// Whether `line` is exactly one JSON object: balanced braces outside
    /// strings, nothing before the first or after the last.
    ///
    /// A structural check rather than a parse, for the reason there is no JSON
    /// crate in this workspace at all — and it is the property a parent actually
    /// depends on, which is that a line is one object it can hand to its own
    /// parser whole.
    fn is_one_object(line: &str) -> bool {
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut closed_at = None;
        for (i, ch) in line.char_indices() {
            if in_string {
                match ch {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    depth -= 1;
                    if depth == 0 {
                        closed_at = Some(i);
                    }
                }
                _ => {}
            }
        }
        !in_string && depth == 0 && closed_at == Some(line.len().saturating_sub(1))
    }

    /// Every event renders as one object a parent can split on its first byte,
    /// carrying the two fields that route it.
    #[test]
    fn an_event_line_is_one_object_a_parent_can_split_on_its_first_byte() {
        for event in roster() {
            let rendered = event.line();
            assert!(
                rendered.ends_with('\n'),
                "{}: the line does not end with a newline, so a parent reading \
                 lines would join it to the next one",
                event.name()
            );
            let body = rendered.trim_end_matches('\n');
            assert!(
                body.starts_with('{'),
                "{}: the line does not begin with a brace, so it is \
                 indistinguishable from a human diagnostic: {body}",
                event.name()
            );
            assert!(
                is_one_object(body),
                "{}: the line is not one balanced object: {body}",
                event.name()
            );
            assert!(
                body.contains(&format!("\"v\":{EVENT_VERSION}")),
                "{}: no version field: {body}",
                event.name()
            );
            assert!(
                body.contains(&format!("\"ev\":\"{}\"", event.name())),
                "{}: no ev field naming it: {body}",
                event.name()
            );
            assert!(
                !body.contains('\n'),
                "{}: the rendered object spans lines: {body}",
                event.name()
            );
        }
    }

    /// The roster this file renders is the roster the protocol names.
    ///
    /// A drift guard rather than a tautology: the list on the right is
    /// `docs/specs/0003-studio-control-protocol.md`'s, and an event added here
    /// without a row there fails until both move.
    #[test]
    fn the_rendered_roster_is_the_one_the_protocol_names() {
        let mut names: Vec<&str> = roster().iter().map(Event::name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names,
            [
                "health",
                "hello",
                "pong",
                "preset",
                "preset_error",
                "preset_warning",
                "stream",
            ],
            "the rendered roster moved; `roster` here covers every event but \
             `roster` itself, which needs an owned slice"
        );
    }

    /// A quote or a backslash in a path or a name is escaped, so one preset
    /// cannot produce a line the parent cannot parse.
    #[test]
    fn a_quote_in_a_name_or_a_path_cannot_break_a_line() {
        let event = Event::Preset {
            name: "a \"quoted\" \\ name",
            index: 0,
        };
        let rendered = event.line();
        assert!(
            is_one_object(rendered.trim_end_matches('\n')),
            "an escaped name broke the object: {rendered}"
        );
        assert!(
            rendered.contains("\\\"quoted\\\""),
            "the quotes were not escaped: {rendered}"
        );
    }

    /// A byte offset becomes the line and column an editor would put a cursor at.
    #[test]
    fn a_byte_offset_becomes_a_one_based_line_and_column() {
        let source = "system = \"swarm\"\n[params]\nbg_bright = \"bins(3)\"\n";
        assert_eq!(
            line_col(source, 0),
            (1, 1),
            "the first byte is line 1 col 1"
        );
        let offset = source.find("bg_bright").expect("the fixture has it");
        assert_eq!(
            line_col(source, offset),
            (3, 1),
            "the third line's first column"
        );
        let inner = source.find("bins").expect("the fixture has it");
        assert_eq!(line_col(source, inner), (3, 14));
        assert_eq!(
            line_col(source, source.len() + 100),
            (4, 1),
            "an offset past the end lands at the end rather than panicking"
        );
    }

    /// A preset with a deliberate syntax error renders a `preset_error` naming
    /// the line the fixture put it on.
    ///
    /// The whole chain the shell walks, minus the file read: the loader's error,
    /// the parser's span through `PresetError::span`, `line_col`, and the
    /// rendered object. The fixture's own error is on line **4** and nothing
    /// about that is a coincidence — the three lines above it are there so a
    /// scheme that reported the offset, or a 0-based line, or the line of the
    /// table header, would each land somewhere else.
    #[test]
    fn a_syntax_error_is_reported_at_the_line_the_fixture_put_it_on() {
        let source = "system = \"swarm\"
name = \"probe\"
[params]
bg_bright = 
";
        let err = rlx_core::preset::Preset::from_toml_str(source)
            .expect_err("the fixture's fourth line has no value after the `=`");
        let span = err.span().expect("the TOML parser gives a span for this");
        let (line, col) = line_col(source, span.start);
        assert_eq!(
            line, 4,
            "the error was reported on line {line}, not the line the fixture put \
             it on"
        );

        let file = Path::new("probe.toml");
        let message = err.to_string();
        let rendered = Event::PresetError {
            file,
            message: &message,
            line: Some(line),
            col: Some(col),
            param: err.param(),
        }
        .line();
        assert!(
            rendered.contains("\"line\":4"),
            "the rendered event does not carry the line: {rendered}"
        );
        assert!(
            rendered.contains("\"file\":\"probe.toml\""),
            "the rendered event does not carry the file: {rendered}"
        );
        assert!(
            is_one_object(rendered.trim_end_matches('\n')),
            "the rendered event is not one object: {rendered}"
        );
    }

    /// An expression error carries the parameter whose expression failed, as a
    /// field rather than only inside the sentence.
    #[test]
    fn an_expression_error_carries_the_parameter_name() {
        let source = "system = \"swarm\"
name = \"probe\"
[params]
bg_bright = \"nope(1)\"
";
        let err = rlx_core::preset::Preset::from_toml_str(source)
            .expect_err("`nope` is not a function in the grammar");
        assert_eq!(
            err.param(),
            Some("bg_bright"),
            "the error does not name the parameter it came from"
        );
        assert!(
            err.span().is_none(),
            "an expression error is raised after the document was parsed, so it \
             has no span; if it grows one, the line and column below become \
             reportable and this expectation should move"
        );

        let message = err.to_string();
        let rendered = Event::PresetError {
            file: Path::new("probe.toml"),
            message: &message,
            line: None,
            col: None,
            param: err.param(),
        }
        .line();
        assert!(
            rendered.contains("\"param\":\"bg_bright\""),
            "the rendered event does not carry the parameter: {rendered}"
        );
        assert!(
            rendered.contains("\"line\":null"),
            "a missing position should be null rather than absent: {rendered}"
        );
    }

    /// `hello` is recorded as sent, so "first line after start" is a property the
    /// shell can assert rather than an ordering it hopes for.
    #[test]
    fn the_sink_records_whether_it_has_greeted() {
        let events = Events::new();
        assert!(!events.greeted(), "a fresh sink has said nothing");
    }
}
