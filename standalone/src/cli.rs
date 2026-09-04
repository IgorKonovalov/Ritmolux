//! The command line: the flag roster, the scanners that judge an argument list
//! before a window exists, and the launch-time resolution of what the flags and
//! `config.toml` disagree about.
//!
//! The roster (`FLAGS`) is a second copy of a fact the scanners encode, and the
//! two are held in step by a test rather than by discipline (ADR-0148). A flag
//! added to a scanner and not to the roster fails
//! `every_scanner_flag_literal_is_rostered`, which reads this module's own
//! source.

use std::path::PathBuf;

use rlx_core::render::Tier;
use standalone::{APP_DIR_NAME, preset_data_root};

use crate::config;

/// Resolve `diagnostics.log` under the per-user app dir (alongside the shared
/// `presets` dir). `None` if the OS data root can't be resolved — the logger
/// then silently no-ops (degrade, never crash — NFR 10).
pub(crate) fn resolve_log_path() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join("diagnostics.log"))
}

/// Resolve `config.toml` under the per-user app dir (same base as the presets
/// and diagnostics log). `None` if the OS data root can't be resolved — the
/// config then loads defaults and hotkey changes apply live but don't persist.
pub(crate) fn resolve_config_path() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join("config.toml"))
}

/// One command-line flag this binary recognizes.
///
/// The roster is a second copy of a fact the scanners below already encode, and
/// `every_scanner_flag_is_rostered` is what holds the two in step rather than
/// discipline (ADR-0148). Keep an entry beside the scanner that reads it.
pub(crate) struct FlagSpec {
    /// Including the leading dashes, and without any `=value` suffix.
    pub(crate) name: &'static str,
    /// Whether the argument after this one is its value rather than a flag of
    /// its own. A following argument that is itself flag-shaped is **not**
    /// consumed: that is the rule `--soak` and `--downbeat-log` already use for
    /// their optional paths, and it is what keeps `--tier --ocs 1.2.3.4:9000`
    /// reporting the typo instead of swallowing it as a tier name. The cost is
    /// that an endpoint or sender name genuinely spelled `--something` is
    /// refused rather than passed through, which is the cheaper of the two
    /// mistakes: every value-taking flag is otherwise a place a typo can hide.
    pub(crate) takes_value: bool,
    /// The flag this one is only read alongside, or `None` when it is read on
    /// every run.
    ///
    /// A conditionally-claimed flag is invisible to the roster gate below: the
    /// scanner that would read it returns early when its companion is absent,
    /// so the flag is walked past as recognized and then read by nothing. That
    /// is the same "running visualizer doing less than it was asked" ADR-0148
    /// exists to refuse, one level down, and stating the dependency here is
    /// what lets `unrecognized_flag` see it (ADR-0155).
    ///
    /// The name must itself be in [`FLAGS`]; `every_requires_names_a_real_flag`
    /// is what holds that, since a typo here would refuse the flag on every run
    /// instead of on none.
    pub(crate) requires: Option<&'static str>,
    /// One line, printed by `--help`. States what the flag does; a `requires`
    /// dependency is rendered from the field above rather than written into
    /// this string, so the two cannot disagree.
    pub(crate) help: &'static str,
}

/// Every flag the binary accepts, in the order `--help` prints them.
///
/// Unconditional, including the `--stream` family: `stream::parse` compiles on
/// every platform and a build without the `spout` feature refuses `--stream`
/// with its own named reason, so a roster that hid those entries behind a `cfg`
/// would turn a documented flag into an unrecognized argument on the builds
/// that explain themselves best.
pub(crate) const FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--help",
        takes_value: false,
        requires: None,
        help: "print this roster and exit",
    },
    FlagSpec {
        name: "--console",
        takes_value: false,
        requires: None,
        help: "open the operator console at launch",
    },
    FlagSpec {
        name: "--list-devices",
        takes_value: false,
        requires: None,
        help: "print the audio capture endpoints and exit (Windows-only)",
    },
    FlagSpec {
        name: "--list-adapters",
        takes_value: false,
        requires: None,
        help: "print the renderer and Spout adapter rosters and exit",
    },
    FlagSpec {
        name: "--input",
        takes_value: true,
        requires: None,
        help: "<loopback|line-in> where audio comes from",
    },
    FlagSpec {
        name: "--device",
        takes_value: true,
        requires: None,
        help: "<name> which capture endpoint to open (see --list-devices)",
    },
    FlagSpec {
        name: "--tier",
        takes_value: true,
        requires: None,
        help: "<floor|rich> pin the quality tier instead of letting the engine pick",
    },
    FlagSpec {
        name: "--osc",
        takes_value: true,
        requires: None,
        help: "<host:port> publish analyzer telemetry as OSC over UDP",
    },
    FlagSpec {
        name: "--soak",
        takes_value: true,
        requires: None,
        help: "[path] write a long-run frame-time trace; bare, it uses a default path",
    },
    FlagSpec {
        name: "--downbeat-log",
        takes_value: true,
        requires: None,
        help: "[path] write the per-beat downbeat decomposition; bare, a default path",
    },
    FlagSpec {
        name: "--stream",
        takes_value: false,
        requires: None,
        help: "run headless and publish every frame as a Spout sender",
    },
    FlagSpec {
        name: "--size",
        takes_value: true,
        requires: Some("--stream"),
        help: "<WxH> published frame size (default 1280x720)",
    },
    FlagSpec {
        name: "--fps",
        takes_value: true,
        requires: Some("--stream"),
        help: "<n> published frame rate (default 60)",
    },
    FlagSpec {
        name: "--gpu",
        takes_value: true,
        requires: None,
        help: "<name|index> which graphics adapter to render on",
    },
    FlagSpec {
        name: "--sender",
        takes_value: true,
        requires: Some("--stream"),
        help: "<name> the published Spout sender name (default Ritmolux)",
    },
    FlagSpec {
        name: "--preset",
        takes_value: true,
        requires: None,
        help: "<name> hold one scene and disable rotation",
    },
    FlagSpec {
        name: "--frames",
        takes_value: true,
        requires: Some("--stream"),
        help: "<n> stop after this many frames",
    },
];

/// Print the flag roster to stdout.
///
/// Called as the **first** statement in `main`, before the event loop, the
/// renderer and any capture client exist. That ordering is the contract: a guard
/// shelling out to `--help` to discover the flag surface must get an answer and
/// an exit, not a window (ADR-0148). `standalone/tests/help_cli.rs` asserts it
/// from outside the process, which is the only place the absence of a window is
/// observable.
pub(crate) fn print_help() {
    print!("{}", help_text());
}

/// The roster as `--help` renders it, built as a value so its content is
/// assertable without a process. The *exit* is not — that is what
/// `standalone/tests/help_cli.rs` spawns the binary for.
///
/// The name column is sized to the longest entry (`--list-adapters`) plus a
/// gap, so adding a longer flag needs the width moved with it.
pub(crate) fn help_text() -> String {
    let mut text =
        String::from("ritmolux — a real-time music visualizer\n\nusage: ritmolux [flags]\n\n");
    for spec in FLAGS {
        // The dependency is rendered from `requires`, never read out of `help`:
        // one field feeds both the printed line and the refusal below, so a
        // flag's documented coupling and its enforced one are the same fact.
        let needs = match spec.requires {
            Some(other) => format!(" [requires {other}]"),
            None => String::new(),
        };
        text.push_str(&format!("  {:<17} {}{}\n", spec.name, spec.help, needs));
    }
    text.push_str("\n-h is a synonym for --help.\n");
    text.push_str("A flag takes its value as `--flag value` or `--flag=value`.\n");
    text.push_str("README.md says what each one is for; config.toml is the persistent form.\n");
    text
}

/// The flag name in `arg` — `--osc` for both `--osc` and `--osc=1.2.3.4:9000` —
/// or `None` when `arg` is not flag-shaped at all.
///
/// A value and a bare `--` are both `None`: the app takes no positionals, so the
/// only thing worth judging is a token that reads as a long flag, and `--` names
/// nothing.
pub(crate) fn flag_name(arg: &str) -> Option<&str> {
    let name = arg.split('=').next().unwrap_or(arg);
    (name.starts_with("--") && name.len() > 2).then_some(name)
}

/// Levenshtein distance between two ASCII flag names.
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

/// The roster entry closest to `name`, or `None` when nothing is near enough to
/// be worth guessing at.
///
/// Two edits is the bar because that is what a transposition costs — `--ocs` for
/// `--osc` is the typo this exists for — and a looser bar starts naming a flag
/// that merely shares three letters with a word.
pub(crate) fn nearest_flag(name: &str) -> Option<&'static FlagSpec> {
    FLAGS
        .iter()
        .map(|spec| (edit_distance(name, spec.name), spec))
        .filter(|&(distance, _)| distance <= 2)
        .min_by_key(|&(distance, _)| distance)
        .map(|(_, spec)| spec)
}

/// One flag-shaped token, as the roster reads it.
pub(crate) enum Claimed {
    /// A token matching a roster entry.
    Known(&'static FlagSpec),
    /// A token matching a roster entry that takes no value, spelled with an
    /// `=value` suffix the scanner for that flag will never see.
    ///
    /// Every scanner claiming a valueless flag compares the whole argument
    /// (`arg == "--stream"`), so `--stream=1` matches nothing while
    /// [`flag_name`] still reduces it to a rostered name. That combination is
    /// exactly the silence ADR-0148 exists to end: the roster walks it past as
    /// recognized, the mode it names never starts, and every flag that depends
    /// on that mode is then read by nothing either.
    Valued(&'static FlagSpec),
    /// A `--`-prefixed token no roster entry names, kept verbatim so the
    /// refusal can echo what was typed rather than a normalized form.
    Unknown(String),
}

/// Walk `args` the way the scanners do, yielding one entry per flag-shaped
/// token.
///
/// The single copy of the stepping rule: a rostered flag's value is stepped
/// over rather than judged, so a device or sender name that happens to be
/// flag-shaped is still its flag's value. Both refusals below read this walk, so
/// they cannot disagree about which tokens were flags (ADR-0148).
pub(crate) fn walk_flags(args: impl Iterator<Item = String>) -> Vec<Claimed> {
    let mut seen = Vec::new();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let Some(name) = flag_name(&arg) else {
            continue;
        };
        let Some(spec) = FLAGS.iter().find(|spec| spec.name == name) else {
            seen.push(Claimed::Unknown(arg.clone()));
            continue;
        };
        if !spec.takes_value && arg.contains('=') {
            seen.push(Claimed::Valued(spec));
            continue;
        }
        seen.push(Claimed::Known(spec));
        // The `=` spelling carries its value inside the same argument, so there
        // is nothing after it to step over.
        if spec.takes_value
            && !arg.contains('=')
            && args.peek().is_some_and(|next| flag_name(next).is_none())
        {
            args.next();
        }
    }
    seen
}

/// The first rostered flag whose `requires` companion is absent, with that
/// companion's name.
///
/// The gap ADR-0148 left: a flag claimed only when another flag is present is
/// walked past as recognized, and then the scanner that would read it returns
/// early. Nothing downstream mentions it again, so the app runs doing less than
/// it was asked with no diagnostic — which is the failure the roster exists to
/// end (ADR-0155).
pub(crate) fn missing_companion(
    args: impl Iterator<Item = String>,
) -> Option<(&'static FlagSpec, &'static str)> {
    let seen = walk_flags(args);
    let present = |name: &str| {
        seen.iter()
            .any(|claimed| matches!(claimed, Claimed::Known(spec) if spec.name == name))
    };
    seen.iter().find_map(|claimed| match claimed {
        Claimed::Known(spec) => spec
            .requires
            .filter(|companion| !present(companion))
            .map(|companion| (*spec, companion)),
        // A malformed occurrence satisfies nothing and asks for nothing: it is
        // refused before this runs, and counting it as present would let
        // `--stream=1 --fps 30` past on the strength of the very token that is
        // wrong.
        Claimed::Valued(_) | Claimed::Unknown(_) => None,
    })
}

/// The first rostered flag that takes no value but was given one.
///
/// See [`Claimed::Valued`] for why this is a silence rather than a parse error
/// anywhere downstream. Returned as the spec so the refusal names the flag
/// rather than the token, which is what tells the operator that the flag exists
/// and the `=` is the mistake.
pub(crate) fn valued_valueless_flag(
    args: impl Iterator<Item = String>,
) -> Option<&'static FlagSpec> {
    walk_flags(args)
        .into_iter()
        .find_map(|claimed| match claimed {
            Claimed::Valued(spec) => Some(spec),
            Claimed::Known(_) | Claimed::Unknown(_) => None,
        })
}

/// The first `--`-prefixed argument no scanner will claim, with the nearest
/// roster entry to it.
///
/// One pass in front of the scanners, which are each a full walk of the argument
/// list looking for a single shape and therefore cannot see an argument nobody
/// wanted (ADR-0148).
pub(crate) fn unrecognized_flag(
    args: impl Iterator<Item = String>,
) -> Option<(String, Option<&'static FlagSpec>)> {
    walk_flags(args)
        .into_iter()
        .find_map(|claimed| match claimed {
            Claimed::Unknown(arg) => {
                let nearest = flag_name(&arg).and_then(nearest_flag);
                Some((arg, nearest))
            }
            Claimed::Known(_) | Claimed::Valued(_) => None,
        })
}

/// A flag whose value is an **optional** path: `--flag=PATH`, `--flag PATH`, or
/// a bare `--flag` that falls back to `default`.
///
/// The bare spelling is why this cannot go through [`flag_value`]: there a
/// missing value is a usage error, and here it is the default. A following
/// argument that is itself flag-shaped belongs to the next flag, not to this
/// one.
fn optional_path_flag(name: &str, default: fn() -> PathBuf) -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(path) = arg
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(PathBuf::from(path));
        }
        if arg == name {
            // An explicit path may follow; otherwise use the default location.
            return match args.next() {
                Some(next) if !next.starts_with("--") => Some(PathBuf::from(next)),
                _ => Some(default()),
            };
        }
    }
    None
}

/// The soak-log path if `--soak` was passed (`--soak <path>` / `--soak=<path>`,
/// or a bare `--soak` for the default under the per-user dir), else `None` so
/// the soak sampler is never created and the render loop is unchanged.
pub(crate) fn parse_soak_arg() -> Option<PathBuf> {
    optional_path_flag("--soak", default_soak_path)
}

/// The downbeat-log path if `--downbeat-log` was passed (`--downbeat-log <path>`
/// / `--downbeat-log=<path>`, or a bare `--downbeat-log` for the default under the
/// per-user dir), else `None` so no logger is created and the frame path is
/// unchanged (Plan 0086 Phase 1).
///
/// The same three shapes `--soak` accepts, and through the same scanner rather
/// than a copy of it: both flags are typed by hand at a capture session, and a
/// mode that took its path differently from the one beside it would be a footgun
/// for the person running both.
pub(crate) fn parse_downbeat_log_arg() -> Option<PathBuf> {
    optional_path_flag("--downbeat-log", default_downbeat_log_path)
}

/// Whether `--console` was passed: open the operator console at launch
/// (ADR-0143).
///
/// A bare presence flag with no value, so there is nothing to reject and no
/// `Result` — unlike `--tier`, a typo here cannot resolve to a *different*
/// console. It ORs with `[console] enabled` rather than overriding it: the flag
/// turns the console on for a run and never off, which is the shape of every
/// other opt-in launch flag here.
pub(crate) fn parse_console_flag() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--console")
}

/// The tier `--tier <name>` / `--tier=<name>` pins, or `None` when the flag is
/// absent (Plan 0044).
///
/// `Err` on a missing or unparseable value. Unlike `RLX_TIER`, a bad `--tier` is
/// a **usage error** rather than something to degrade past: it was typed for this
/// run, so silently starting on another tier would answer the wrong question.
pub(crate) fn parse_tier_arg() -> Result<Option<Tier>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if let Some(inline) = arg.strip_prefix("--tier=") {
            Some(inline.to_owned())
        } else if arg == "--tier" {
            Some(args.next().unwrap_or_default())
        } else {
            None
        };
        if let Some(value) = value {
            return Tier::from_name(&value)
                .map(Some)
                .ok_or_else(|| format!("--tier `{value}`: expected `floor` or `rich`"));
        }
    }
    Ok(None)
}

/// The value of a single-valued flag, in both spellings, or `None` when the
/// flag is absent.
///
/// The scanner the windowed `--gpu` and `--preset` share. Both are also read by
/// `stream::parse` on the headless path; this is the window's reader, and the
/// two never run in the same process because `--stream` decides which mode this
/// binary is before either is consulted (ADR-0155).
pub(crate) fn windowed_flag(name: &'static str) -> Result<Option<String>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = flag_value(&arg, name, &mut args) {
            return value.map(Some);
        }
    }
    Ok(None)
}

/// The value of `--name value` / `--name=value` when `arg` is that flag, else
/// `None`. The spaced spelling consumes the next argument, which is why the
/// iterator is threaded through rather than re-scanned.
///
/// The inner `Err` is the flag with nothing after it at all. It is a usage error
/// rather than an empty value, because an empty value is not inert: `--device`
/// reads `""` as the mode's default endpoint, which is the opposite of asking
/// for a device by name.
pub(crate) fn flag_value(
    arg: &str,
    name: &str,
    args: &mut impl Iterator<Item = String>,
) -> Option<Result<String, String>> {
    if let Some(inline) = arg
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('='))
    {
        return Some(Ok(inline.to_owned()));
    }
    if arg != name {
        return None;
    }
    Some(
        args.next()
            .ok_or_else(|| format!("{name}: expected a value")),
    )
}

/// The `--input <mode>` / `--device <name>` overrides, in both the spaced and
/// the `=` spelling `--soak` and `--tier` already accept.
///
/// `Err` on an `--input` value that names no mode. Like `--tier` and unlike
/// `RLX_TIER`, a bad flag is a **usage error** rather than something to degrade
/// past: it was typed for this run, so starting on another input would answer
/// the wrong question. A `--device` naming an absent endpoint is *not* an error
/// — the capture layer degrades to the mode's default endpoint and says so, and
/// a flag must not be stricter about the world than about its own spelling.
pub(crate) fn parse_input_args() -> Result<(Option<config::InputMode>, Option<String>), String> {
    parse_input_args_from(std::env::args().skip(1))
}

/// [`parse_input_args`]'s rule as a pure function of the argument list, so both
/// spellings are testable without a process.
pub(crate) fn parse_input_args_from(
    args: impl Iterator<Item = String>,
) -> Result<(Option<config::InputMode>, Option<String>), String> {
    let mut args = args.peekable();
    let mut mode = None;
    let mut device = None;
    while let Some(arg) = args.next() {
        if let Some(value) = flag_value(&arg, "--input", &mut args) {
            let value = value?;
            mode =
                Some(config::InputMode::from_name(&value).ok_or_else(|| {
                    format!("--input `{value}`: expected `loopback` or `line-in`")
                })?);
        } else if let Some(value) = flag_value(&arg, "--device", &mut args) {
            let value = value?;
            // Both spellings, so `--device=` is refused for the same reason a
            // trailing `--device` is: it selects the default endpoint while
            // reading as a request for a named one.
            if value.trim().is_empty() {
                return Err("--device: expected an endpoint name (see --list-devices)".to_owned());
            }
            device = Some(value);
        }
    }
    Ok((mode, device))
}

/// Which source decided the capture selection, so a surprising input is
/// traceable to what set it — the tier-source shape ADR-0045 established, minus
/// the environment level (Plan 0130 says why there is no `RLX_INPUT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputSource {
    /// `--input` / `--device` on the command line.
    Flag,
    /// `config.toml`'s `[input]` section moved it off the built-in.
    Config,
    /// Nothing chose it: loopback of the default render endpoint.
    Default,
}

impl InputSource {
    /// How to name this source in a log line.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            InputSource::Flag => "--input/--device",
            InputSource::Config => "config.toml",
            InputSource::Default => "default",
        }
    }
}

/// Resolve the capture selection: `--input` / `--device` over `[input]`.
///
/// **Each flag overrides its own field**, so `--device` alone keeps the
/// configured mode and `--input` alone keeps the configured device name. A name
/// that belongs to the other dataflow then matches nothing and degrades to that
/// mode's default endpoint with a stderr note — announced and self-correcting,
/// which is a better answer than silently discarding what the operator
/// configured.
///
/// Pure: every source arrives already parsed, so the precedence rule is testable
/// without touching the process environment.
pub(crate) fn resolve_input(
    mode: Option<config::InputMode>,
    device: Option<String>,
    config: &config::Input,
) -> (config::Input, InputSource) {
    let from_flag = mode.is_some() || device.is_some();
    let resolved = config::Input {
        mode: mode.unwrap_or(config.mode),
        device: device.unwrap_or_else(|| config.device.clone()),
    };
    let built_in = config::Input::default();
    let source = if from_flag {
        InputSource::Flag
    } else if resolved.mode == built_in.mode && resolved.device == built_in.device {
        InputSource::Default
    } else {
        InputSource::Config
    };
    (resolved, source)
}

/// The `--osc <host:port>` override, in both the spaced and the `=` spelling.
///
/// `Err` on the flag with nothing after it. An empty value is refused for the
/// same reason `--device=` is: it reads as a request and means nothing.
pub(crate) fn parse_osc_arg() -> Result<Option<String>, String> {
    parse_osc_arg_from(std::env::args().skip(1))
}

/// [`parse_osc_arg`]'s rule as a pure function of the argument list.
pub(crate) fn parse_osc_arg_from(
    args: impl Iterator<Item = String>,
) -> Result<Option<String>, String> {
    let mut args = args.peekable();
    let mut target = None;
    while let Some(arg) = args.next() {
        if let Some(value) = flag_value(&arg, "--osc", &mut args) {
            let value = value?;
            if value.trim().is_empty() {
                return Err("--osc: expected a target as host:port".to_owned());
            }
            target = Some(value);
        }
    }
    Ok(target)
}

/// Resolve the telemetry target: `--osc` over `[osc]`. `None` means the sink
/// stays off and no socket is bound.
///
/// **The flag both aims the sink and turns it on**, which is what makes
/// `--osc 10.0.0.4:7700` a complete instruction rather than one that also needs
/// `enabled = true` in a file. `enabled = false` in the config cannot veto a
/// flag typed for this run — the flag is the more specific statement, the same
/// precedence `--input` and `--tier` already have.
pub(crate) fn resolve_osc(flag: Option<String>, config: &config::Osc) -> Option<(String, u32)> {
    match flag {
        Some(target) => Some((target, config.rate_hz)),
        None if config.enabled => Some((config.target.clone(), config.rate_hz)),
        None => None,
    }
}

/// Default soak-log location: under the per-user app dir, or `soak.log` in the
/// current directory if that can't be resolved — so `--soak` always logs
/// somewhere.
pub(crate) fn default_soak_path() -> PathBuf {
    preset_data_root()
        .map(|root| root.join(APP_DIR_NAME).join("soak.log"))
        .unwrap_or_else(|| PathBuf::from("soak.log"))
}

/// Default per-beat downbeat-log location: under the per-user app dir, or
/// `downbeat.log` in the current directory if that can't be resolved — so a bare
/// `--downbeat-log` always logs somewhere.
pub(crate) fn default_downbeat_log_path() -> PathBuf {
    preset_data_root()
        .map(|root| root.join(APP_DIR_NAME).join("downbeat.log"))
        .unwrap_or_else(|| PathBuf::from("downbeat.log"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        FLAGS, InputSource, config, help_text, missing_companion, parse_input_args_from,
        parse_osc_arg_from, resolve_input, resolve_osc, unrecognized_flag, valued_valueless_flag,
    };

    /// `--osc` in both spellings, and the empty value refused for the same
    /// reason `--device=` is: it reads as a request and names nothing.
    #[test]
    fn both_osc_flag_spellings_parse() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        assert_eq!(
            parse_osc_arg_from(argv(&["--osc", "192.168.0.1:9000"]).into_iter()),
            Ok(Some("192.168.0.1:9000".to_owned()))
        );
        assert_eq!(
            parse_osc_arg_from(argv(&["--osc=192.168.0.1:9000"]).into_iter()),
            Ok(Some("192.168.0.1:9000".to_owned()))
        );
        // An unaccompanied flag stays absent; the resolver decides what that means.
        assert_eq!(
            parse_osc_arg_from(argv(&["--soak", "--tier=floor"]).into_iter()),
            Ok(None)
        );
        assert!(parse_osc_arg_from(argv(&["--osc="]).into_iter()).is_err());
        assert!(parse_osc_arg_from(argv(&["--osc"]).into_iter()).is_err());
    }

    /// **The flag both aims the sink and turns it on**, and `enabled = false` in
    /// a file cannot veto it — otherwise `--osc <target>` would silently do
    /// nothing on the one machine whose config had ever been written. With no
    /// flag, the config decides, and the built-in default is off: a machine that
    /// never asked for a lighting rig binds no socket.
    #[test]
    fn the_osc_flag_overrides_the_config_and_enables_the_sink() {
        let off = config::Osc::default();
        assert!(!off.enabled, "the built-in default must be off");
        assert_eq!(resolve_osc(None, &off), None);

        assert_eq!(
            resolve_osc(Some("10.0.0.4:7700".to_owned()), &off),
            Some(("10.0.0.4:7700".to_owned(), off.rate_hz)),
            "an explicit flag did not beat `enabled = false`"
        );

        let on = config::Osc {
            enabled: true,
            target: "192.168.1.101:7000".to_owned(),
            rate_hz: 30,
        };
        assert_eq!(
            resolve_osc(None, &on),
            Some(("192.168.1.101:7000".to_owned(), 30))
        );
        // The flag moves the target and leaves the cadence to the file, which is
        // the only key it has no spelling for.
        assert_eq!(
            resolve_osc(Some("10.0.0.4:7700".to_owned()), &on),
            Some(("10.0.0.4:7700".to_owned(), 30))
        );
    }

    /// Both spellings of both flags, and the fact that an absent flag stays
    /// absent rather than resolving to a default value here — the resolver, not
    /// the parser, is what decides what an absent flag means.
    #[test]
    fn both_flag_spellings_parse() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        assert_eq!(
            parse_input_args_from(
                argv(&["--input", "line-in", "--device", "ZOOM AMS-22"]).into_iter()
            ),
            Ok((
                Some(config::InputMode::LineIn),
                Some("ZOOM AMS-22".to_owned())
            ))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--input=line-in", "--device=ZOOM AMS-22"]).into_iter()),
            Ok((
                Some(config::InputMode::LineIn),
                Some("ZOOM AMS-22".to_owned())
            ))
        );
        // Case-insensitive, like `--tier`, and unaccompanied flags stay `None`.
        assert_eq!(
            parse_input_args_from(argv(&["--soak", "--input", "LOOPBACK"]).into_iter()),
            Ok((Some(config::InputMode::Loopback), None))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--device=default"]).into_iter()),
            Ok((None, Some("default".to_owned())))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--fullscreen"]).into_iter()),
            Ok((None, None))
        );

        // A value that names no mode is a usage error naming what it saw, not a
        // silent fall-through to loopback.
        let err = parse_input_args_from(argv(&["--input", "lineout"]).into_iter())
            .expect_err("`lineout` is not an input mode");
        assert!(err.contains("--input") && err.contains("lineout"), "{err}");
        // Including the spelling that swallows the next argument when there is
        // none to swallow: an empty value is still a value that named no mode.
        assert!(parse_input_args_from(argv(&["--input"]).into_iter()).is_err());
    }

    /// **A `--device` with no name is a usage error, in both spellings.** An
    /// empty value is not inert: `start_capture` reads `""` as the mode's
    /// default endpoint, so the flag would quietly select the opposite of what
    /// asking for a device by name means.
    #[test]
    fn a_device_flag_with_no_name_is_a_usage_error() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        // Trailing, with nothing to swallow.
        let err = parse_input_args_from(argv(&["--device"]).into_iter())
            .expect_err("a bare `--device` selected something");
        assert!(err.contains("--device"), "{err}");
        // The `=` spelling of the same mistake, and a value that is only space.
        assert!(parse_input_args_from(argv(&["--device="]).into_iter()).is_err());
        assert!(parse_input_args_from(argv(&["--device", "   "]).into_iter()).is_err());

        // The explicit word still parses: `default` is a real selection, and it
        // is the one an operator types to undo a `--device` in a launcher.
        assert_eq!(
            parse_input_args_from(argv(&["--device", "default"]).into_iter()),
            Ok((None, Some("default".to_owned())))
        );
    }

    /// **The flags win over `config.toml`, one field at a time.** This is the
    /// precedence ADR-0142 mirrors off `--tier`, and the per-field half is what
    /// lets `--device` alone keep the configured mode.
    #[test]
    fn the_flags_override_the_config_field_by_field() {
        let configured = config::Input {
            mode: config::InputMode::Loopback,
            device: "Speakers (Realtek)".to_owned(),
        };

        // Both flags: neither configured field survives, and the flag is named
        // as the source.
        let (input, source) = resolve_input(
            Some(config::InputMode::LineIn),
            Some("Line (ZOOM AMS-22 Audio)".to_owned()),
            &configured,
        );
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(input.device, "Line (ZOOM AMS-22 Audio)");
        assert_eq!(source, InputSource::Flag);

        // `--input` alone keeps the configured device name; `--device` alone
        // keeps the configured mode.
        let (input, source) = resolve_input(Some(config::InputMode::LineIn), None, &configured);
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(
            input.device, configured.device,
            "the config device was lost"
        );
        assert_eq!(source, InputSource::Flag);

        let (input, source) = resolve_input(None, Some("Line (ZOOM)".to_owned()), &configured);
        assert_eq!(input.mode, configured.mode, "the config mode was lost");
        assert_eq!(input.device, "Line (ZOOM)");
        assert_eq!(source, InputSource::Flag);
    }

    /// With no flags the config decides, and the source distinguishes a config
    /// that moved the selection from one that merely restates the built-in —
    /// which is what keeps the startup line off a run nothing chose.
    #[test]
    fn without_flags_the_config_decides_and_the_built_in_is_named_as_such() {
        let configured = config::Input {
            mode: config::InputMode::LineIn,
            device: "Line (ZOOM AMS-22 Audio)".to_owned(),
        };
        let (input, source) = resolve_input(None, None, &configured);
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(input.device, configured.device);
        assert_eq!(source, InputSource::Config);

        let (input, source) = resolve_input(None, None, &config::Input::default());
        assert_eq!(input.mode, config::InputMode::Loopback);
        assert_eq!(input.device, "default");
        assert_eq!(source, InputSource::Default);

        // A config that spells out the built-in is the same selection, so it is
        // reported the same way rather than as a choice someone made.
        let spelled_out = config::Input {
            mode: config::InputMode::Loopback,
            device: "default".to_owned(),
        };
        assert_eq!(
            resolve_input(None, None, &spelled_out).1,
            InputSource::Default
        );
    }

    /// Every source renders to a distinct, non-empty name: the startup line
    /// exists to say *what* set a surprising input, so two sources that print
    /// the same word would defeat it.
    #[test]
    fn the_three_input_sources_are_distinguishable() {
        let names = [
            InputSource::Flag.as_str(),
            InputSource::Config.as_str(),
            InputSource::Default.as_str(),
        ];
        for name in names {
            assert!(!name.is_empty());
        }
        assert_ne!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert_ne!(names[0], names[2]);
    }

    /// [`unrecognized_flag`] with the matched spec reduced to its name, so an
    /// expectation reads as the pair the operator is shown.
    fn refused(args: &[&str]) -> Option<(String, Option<&'static str>)> {
        let argv = args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        unrecognized_flag(argv.into_iter()).map(|(arg, near)| (arg, near.map(|spec| spec.name)))
    }

    /// **The reduction from the entry that produced this gate.** A misspelt
    /// `--osc` is a running visualizer with a dark rig, so the refusal has to
    /// name the flag that was meant, not merely reject the one that was typed.
    #[test]
    fn a_misspelt_flag_is_refused_and_the_nearest_one_named() {
        assert_eq!(
            refused(&["--ocs", "127.0.0.1:9000"]),
            Some(("--ocs".to_owned(), Some("--osc")))
        );
        assert_eq!(
            refused(&["--teir=floor"]),
            Some(("--teir=floor".to_owned(), Some("--tier")))
        );

        // Nothing in the roster is within two edits of this, so a guess would
        // name a flag sharing nothing with what was typed.
        assert_eq!(
            refused(&["--definitely-not-a-flag"]),
            Some(("--definitely-not-a-flag".to_owned(), None))
        );
    }

    /// [`missing_companion`] reduced to the pair of names the operator is
    /// shown.
    fn orphaned(args: &[&str]) -> Option<(&'static str, &'static str)> {
        let argv = args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        missing_companion(argv.into_iter()).map(|(spec, needs)| (spec.name, needs))
    }

    /// **The reduction from the entry that produced this arm.** Without this
    /// refusal, `--gpu 1` and no `--stream` starts the app and renders on
    /// whatever adapter it would have picked anyway, saying nothing: the roster
    /// walks it past as recognized and `stream::parse` returns before reading
    /// it (design-backlog 0167).
    #[test]
    fn a_flag_whose_companion_is_absent_is_refused_and_both_are_named() {
        assert_eq!(orphaned(&["--fps", "30"]), Some(("--fps", "--stream")));
        assert_eq!(orphaned(&["--sender=rig"]), Some(("--sender", "--stream")));

        // With the companion present each is read by the scanner that claims
        // it, which is the whole condition.
        assert_eq!(orphaned(&["--stream", "--fps", "30"]), None);
        // Order does not matter: the walk collects before it judges.
        assert_eq!(orphaned(&["--fps", "30", "--stream"]), None);
    }

    /// **`--gpu` and `--preset` carry no dependency**, because both reach the
    /// windowed path (ADR-0155). A regression putting either back behind
    /// `--stream` is a flag refused on a working invocation, and it is
    /// unfalsifiable anywhere else: `help_cli.rs` spawns the binary for
    /// `--preset` only, and a `--gpu` that has to open a wgpu device to prove
    /// itself is not a unit test.
    #[test]
    fn the_two_windowed_flags_carry_no_dependency() {
        assert_eq!(orphaned(&["--gpu", "1"]), None);
        assert_eq!(orphaned(&["--preset", "Clifford"]), None);
    }

    /// [`valued_valueless_flag`] reduced to the name the operator is shown.
    fn over_valued(args: &[&str]) -> Option<&'static str> {
        let argv = args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        valued_valueless_flag(argv.into_iter()).map(|spec| spec.name)
    }

    /// **A valueless flag given a value is the silence both other gates miss.**
    /// `--stream=1` reduces to a rostered name, so it is not unrecognized, and
    /// it counts as a `--stream` occurrence, so the companion check passes it
    /// too — while every scanner that claims `--stream` compares the whole
    /// argument and sees nothing. The app then starts windowed and ignores the
    /// rest of the stream family without a word, which is the exact failure
    /// ADR-0155 exists to end.
    #[test]
    fn a_valueless_flag_given_a_value_is_refused() {
        for name in [
            "--stream",
            "--help",
            "--console",
            "--list-devices",
            "--list-adapters",
        ] {
            assert_eq!(
                over_valued(&[&format!("{name}=1")]),
                Some(name),
                "`{name}=1` was not refused"
            );
        }

        // The whole shape from the review. `unrecognized_flag` cannot see it:
        // the token reduces to a rostered name. `missing_companion` sees only
        // the consequence — `--fps` with no usable `--stream` — which names the
        // wrong token, and is why `main` runs this check first.
        assert_eq!(
            over_valued(&["--stream=1", "--fps", "30"]),
            Some("--stream")
        );
        assert_eq!(refused(&["--stream=1", "--fps", "30"]), None);
        assert_eq!(
            orphaned(&["--stream=1", "--fps", "30"]),
            Some(("--fps", "--stream")),
            "a malformed `--stream` must not satisfy another flag's dependency"
        );

        // A value-taking flag is unaffected in either spelling, and a correct
        // valueless flag is not convicted by a `=` somewhere else on the line.
        assert_eq!(over_valued(&["--stream", "--fps=30"]), None);
        assert_eq!(over_valued(&["--stream", "--fps", "30"]), None);
    }

    /// A flag-shaped **value** is not its own flag, so it cannot satisfy
    /// another flag's dependency. `--device` refuses to swallow a flag-shaped
    /// token, so the `--stream` here is a real occurrence; a `--device=--stream`
    /// carries it inside the value and is not.
    #[test]
    fn a_companion_hiding_in_a_value_does_not_count() {
        assert_eq!(
            orphaned(&["--device=--stream", "--fps", "30"]),
            Some(("--fps", "--stream")),
            "`--stream` inside an `=` value is text, not a flag occurrence"
        );
    }

    /// **A typo in `requires` would refuse its flag on every run**, not on
    /// none, so it is the one field here whose failure mode is worse than the
    /// silence it replaces. Nothing else checks it.
    #[test]
    fn every_requires_names_a_real_flag() {
        for spec in FLAGS {
            let Some(companion) = spec.requires else {
                continue;
            };
            assert!(
                FLAGS.iter().any(|other| other.name == companion),
                "`{}` requires `{companion}`, which is not itself in FLAGS",
                spec.name
            );
            assert_ne!(
                spec.name, companion,
                "`{}` requires itself, which no argument list can satisfy",
                spec.name
            );
        }
    }

    /// **Every rostered flag is claimed, in both spellings.** The gate is
    /// additive: a roster that refuses one of its own flags stops a working
    /// invocation, which is a worse failure than the silence it replaces.
    #[test]
    fn every_rostered_flag_is_claimed_in_both_spellings() {
        for spec in FLAGS {
            let spaced: Vec<String> = if spec.takes_value {
                vec![spec.name.to_owned(), "value".to_owned()]
            } else {
                vec![spec.name.to_owned()]
            };
            assert_eq!(
                unrecognized_flag(spaced.into_iter()).map(|(arg, _)| arg),
                None,
                "the roster refused its own `{}`",
                spec.name
            );
            if spec.takes_value {
                let inline = vec![format!("{}=value", spec.name)];
                assert_eq!(
                    unrecognized_flag(inline.into_iter()).map(|(arg, _)| arg),
                    None,
                    "the roster refused `{}=value`",
                    spec.name
                );
            }
        }
    }

    /// **A value is stepped over; a flag-shaped token never is.** An endpoint
    /// name is operator-supplied text and must not be judged as a flag, but a
    /// token that reads as one is judged even where a value was expected —
    /// otherwise the value-taking flags, which are most of the roster, would
    /// each be a place a typo could hide.
    #[test]
    fn a_value_is_stepped_over_and_a_flag_shaped_token_is_still_judged() {
        assert_eq!(
            refused(&[
                "--device",
                "Line (ZOOM AMS-22 Audio)",
                "--ocs",
                "127.0.0.1:9000"
            ]),
            Some(("--ocs".to_owned(), Some("--osc")))
        );
        assert_eq!(
            refused(&["--device", "--ocs"]),
            Some(("--ocs".to_owned(), Some("--osc")))
        );
        assert_eq!(
            refused(&["--console", "--weird-endpoint-name"]),
            Some(("--weird-endpoint-name".to_owned(), None))
        );
    }

    /// **A bare `--tier` does not swallow the flag after it.** The scanners take
    /// an optional value by refusing a flag-shaped one, and a gate that consumed
    /// unconditionally would hide exactly the typo it exists to report.
    #[test]
    fn an_omitted_value_does_not_hide_the_next_typo() {
        assert_eq!(
            refused(&["--tier", "--ocs", "127.0.0.1:9000"]),
            Some(("--ocs".to_owned(), Some("--osc")))
        );
        assert_eq!(refused(&["--soak", "--console"]), None);
    }

    /// **Only long flags are judged.** The app takes no positionals, so a bare
    /// word is not an argument to reject, and `--` names nothing.
    #[test]
    fn a_positional_and_a_bare_double_dash_are_not_flags() {
        assert_eq!(refused(&["nonsense", "--"]), None);
    }

    /// Every flag name a scanner compares an argument against, read out of the
    /// module's own source.
    ///
    /// A flag literal is the **whole** string — `"--osc"`, or the `=` spelling
    /// `"--tier="` — which is what separates a comparison from prose that
    /// mentions a flag (`"--stream: no preset named …"`, and `stream.rs`'s line
    /// saying `--list-presets` is not a flag). The scan stops at the test module,
    /// where a `--`-prefixed literal is an input rather than a flag the binary
    /// claims.
    fn scanner_flag_literals(source: &str) -> Vec<String> {
        let body = source.split("#[cfg(test)]").next().unwrap_or(source);
        let bytes = body.as_bytes();
        let mut found = Vec::new();
        let mut i = 0;
        while let Some(offset) = body[i..].find("\"--") {
            // Past the opening quote; the name itself starts at the dashes.
            let start = i + offset + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end] == b'-'
                    || bytes[end].is_ascii_lowercase()
                    || bytes[end].is_ascii_digit())
            {
                end += 1;
            }
            let mut close = end;
            if close < bytes.len() && bytes[close] == b'=' {
                close += 1;
            }
            // A bare `"--"` is the scanners' own test for "the next argument is
            // a flag, so it is not my value", not a flag name.
            if end > start + 2 && close < bytes.len() && bytes[close] == b'"' {
                found.push(body[start..end].to_owned());
            }
            i = start + 2;
        }
        found
    }

    /// **ADR-0148's drift gate.** The roster is a second copy of a fact the
    /// scanners encode, and this is what keeps them in step: a flag added to a
    /// scanner and not to the roster fails here rather than shipping as an
    /// argument the binary accepts and `--help` does not mention.
    ///
    /// One-directional by construction — it cannot assert that every roster
    /// entry is still reachable, so a retired flag can linger in `--help`.
    #[test]
    fn every_scanner_flag_literal_is_rostered() {
        let sources = [
            ("cli.rs", include_str!("cli.rs")),
            ("run.rs", include_str!("run.rs")),
            ("stream.rs", include_str!("stream.rs")),
        ];
        for (file, source) in sources {
            let literals = scanner_flag_literals(source);
            // A lexer that silently stopped matching would make this test pass
            // by finding nothing, which is the one way a drift gate fails
            // quietly.
            assert!(
                literals.len() >= 5,
                "the scan found only {} flag literals in {file}; it has stopped reading the source",
                literals.len()
            );
            for literal in literals {
                let name = literal.trim_end_matches('=');
                assert!(
                    FLAGS.iter().any(|spec| spec.name == name),
                    "`{name}` is compared against an argument in {file} and is not in FLAGS, \
                     so the binary accepts a flag --help does not mention"
                );
            }
        }
    }

    /// **The advertised sender default is the actual one.** `--sender`'s help
    /// spells the default in prose while [`crate::stream::DEFAULT_SENDER`] holds it as
    /// a value, and nothing but this test couples them — so the roster can
    /// advertise a name no run will ever publish.
    #[test]
    fn the_help_text_names_the_real_sender_default() {
        let text = help_text();
        assert!(
            text.contains(crate::stream::DEFAULT_SENDER),
            "--help advertises a sender default that is not `{}`: {text:?}",
            crate::stream::DEFAULT_SENDER
        );
    }

    /// **`--help` prints the whole roster.** The roster is what an operator
    /// reaches for to check a spelling, so an entry it does not print is an
    /// entry that does not exist as far as anyone outside the source is
    /// concerned.
    #[test]
    fn the_help_text_prints_every_rostered_flag() {
        let text = help_text();
        for spec in FLAGS {
            assert!(
                text.contains(spec.name),
                "--help does not mention `{}`",
                spec.name
            );
            assert!(
                text.contains(spec.help),
                "--help does not carry the help line for `{}`",
                spec.name
            );
        }
        assert!(text.contains("-h"), "--help does not name its own synonym");
    }

    /// **`--help` renders each dependency once, from the field that enforces
    /// it.** The coupling is stated once, in `requires`, rather than as prose
    /// inside each companion's `help` string where nothing holds it to what the
    /// code does; a flag that changes its dependency cannot disagree with its own
    /// documentation, because both read `requires`.
    #[test]
    fn the_help_text_states_each_dependency_once() {
        let text = help_text();
        for spec in FLAGS {
            let line = text
                .lines()
                .find(|line| line.trim_start().starts_with(spec.name))
                .unwrap_or_else(|| panic!("--help has no line for `{}`", spec.name));
            match spec.requires {
                Some(companion) => {
                    assert!(
                        line.contains(&format!("[requires {companion}]")),
                        "`{}` does not state its dependency: {line}",
                        spec.name
                    );
                    assert_eq!(
                        line.matches(companion).count(),
                        1,
                        "`{}` names `{companion}` more than once: {line}",
                        spec.name
                    );
                }
                None => assert!(
                    !line.contains("[requires "),
                    "`{}` states a dependency it does not have: {line}",
                    spec.name
                ),
            }
            assert!(
                !spec.help.contains("[requires "),
                "`{}` writes its dependency into `help` instead of `requires`",
                spec.name
            );
        }
    }
}
