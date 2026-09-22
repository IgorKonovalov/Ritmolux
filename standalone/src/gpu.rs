//! Resolving one operator-supplied GPU name into two independent choices
//! (ADR-0146).
//!
//! The renderer and the Spout sender select their adapter through different
//! APIs with different rosters and no identifier in common that wgpu exposes.
//! Rather than match an adapter across the two, **the operator's string is the
//! common key**: each side looks it up in its own roster. That is enough
//! because the two are not coupled on the CPU pixel path — the frame goes from
//! the render device to system RAM and only then into the sender's own D3D11
//! device, so the sender's adapter is a correctness constraint (a receiver can
//! only open a sender on the GPU it renders with) while the renderer's is a
//! frame-rate one.
//!
//! Everything here is a pure function over rosters passed in, so it is testable
//! with no GPU, no audio device and no Spout SDK.

use rlx_core::render::{AdapterChoice, RenderError};

/// Which of the window's two carriers named its adapter, so the startup line
/// can say where the choice came from — the shape `TierSource` gives the tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterSource {
    /// `--gpu <name|index>` on the command line.
    Flag,
    /// `config.toml`'s `[output] gpu`.
    Config,
    /// Neither named one: the window's default preference.
    Default,
}

/// How the `renderer adapter:` line names the unflagged, unstored request —
/// the words both the plain default and a fall-back onto it print. Named as a
/// preference rather than a pin, because that is what it is: wgpu chose the
/// adapter under a preference, and an operator reading the line should not
/// take it for something they or a file asked for.
const DEFAULT_WORD: &str = "default: high performance";

impl AdapterSource {
    /// The suffix the `renderer adapter:` line carries, naming the carrier.
    pub fn note(self) -> String {
        match self {
            AdapterSource::Flag => " (pinned by --gpu)".to_owned(),
            AdapterSource::Config => " (from config.toml [output] gpu)".to_owned(),
            AdapterSource::Default => format!(" ({DEFAULT_WORD})"),
        }
    }
}

/// Resolve the **window's** adapter across its two carriers, highest
/// precedence first: `--gpu`, then `[output] gpu`, then the default preference
/// — the precedence `--tier` has over `[quality] tier` (ADR-0246).
///
/// Pure: both carriers arrive already read, so the rule is testable without
/// a command line or a file, and a carrier that is absent is simply `None`
/// here.
pub fn resolve_window_choice(
    flag: Option<&str>,
    config: Option<&str>,
) -> (AdapterChoice, AdapterSource) {
    match (flag, config) {
        (Some(raw), _) => (named_or_index(raw), AdapterSource::Flag),
        (None, Some(raw)) => (named_or_index(raw), AdapterSource::Config),
        (None, None) => (window_choice(None), AdapterSource::Default),
    }
}

/// Whether a window that could not get the adapter `source` named may start
/// on the default preference instead — or must refuse to start.
///
/// **The two carriers deliberately differ, and this is the one place the
/// difference is decided.** A stored `[output] gpu` outlives the machine state
/// that made it valid — an undocked laptop, a driver update, an eGPU
/// unplugged — and a show binary that will not start is the wrong failure for
/// a persisted preference, so the file falls back. `--gpu` was typed for this
/// run and can be wrong loudly, so the flag keeps ADR-0155's hard refusal.
///
/// Only the three *resolution* errors fall back: the adapter is absent, the
/// name is ambiguous, or the adapter cannot present to this window. A device
/// that enumerates and then fails to open is the failure a default launch
/// would hit too, and falling back would only hide which one it was.
pub fn fallback_permitted(source: AdapterSource, err: &RenderError) -> bool {
    source == AdapterSource::Config
        && matches!(
            err,
            RenderError::NoSuchAdapter { .. }
                | RenderError::AmbiguousAdapter { .. }
                | RenderError::AdapterCannotPresent { .. }
        )
}

/// The `renderer adapter:` suffix after a fall-back, naming both what the
/// file asked for and that the default was taken instead.
pub fn fallback_note(wanted: &str) -> String {
    format!(" ({DEFAULT_WORD}; \"{wanted}\" from config.toml [output] gpu did not resolve)")
}

/// What the sender should do about its adapter, and what to tell the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderAdapter {
    /// Use this index. Either the operator named it, or it was matched from the
    /// renderer's adapter.
    Pinned(u32),
    /// Let D3D11 pick, because nothing could be matched. Carries the line to
    /// print: a silent fall-back to the default adapter is the wrong GPU on a
    /// hybrid machine, and is exactly the failure this module exists to avoid.
    Default {
        /// Why the match failed, phrased for the operator.
        reason: String,
    },
}

/// Which adapter the **`--stream`** renderer should ask for.
///
/// `None` resolves to [`AdapterChoice::HighPerformance`] rather than the wgpu
/// default: a live source that renders on the power-saving GPU reports that
/// GPU's frame rate as the engine's cost.
pub fn renderer_choice(wanted: Option<&str>) -> AdapterChoice {
    match wanted {
        None => AdapterChoice::HighPerformance,
        Some(raw) => named_or_index(raw),
    }
}

/// Which adapter the **window** should ask for.
///
/// Spelled exactly as [`renderer_choice`] spells it, `None` included: the
/// unflagged window prefers the high-performance adapter, as the stream always
/// has (ADR-0246). On a hybrid machine wgpu's plain default is the
/// power-saving part, and a show that comes up at a sixth of the hardware's
/// frame rate unless the operator knows a flag exists is the failure this
/// arm ends.
///
/// **This is the standalone's preference and not the engine's.**
/// `RendererOptions::default()` still carries [`AdapterChoice::Default`], so
/// the foobar shim's window, which builds its renderer through the C ABI with
/// no flag surface, asks for exactly what it always asked for.
///
/// **The two `None` arms must agree** —
/// `the_window_and_the_stream_agree_when_unflagged` is what holds them
/// together, so one path cannot quietly move off the other.
pub fn window_choice(wanted: Option<&str>) -> AdapterChoice {
    renderer_choice(wanted)
}

/// A bare integer is a roster position; anything else is a name to match.
fn named_or_index(raw: &str) -> AdapterChoice {
    match raw.parse::<usize>() {
        Ok(index) => AdapterChoice::Index(index),
        Err(_) => AdapterChoice::Named(raw.to_owned()),
    }
}

/// Which adapter the Spout sender should live on.
///
/// `roster` is the sender API's own adapter list, in its own index order —
/// never assumed to agree with the renderer's. With no operator string the
/// sender **follows the renderer by name**, which is the whole of the
/// no-flag default; where that match fails it says so rather than reverting
/// quietly.
pub fn sender_adapter(
    wanted: Option<&str>,
    renderer_name: &str,
    roster: &[String],
) -> Result<SenderAdapter, String> {
    let Some(raw) = wanted else {
        return Ok(follow_renderer(renderer_name, roster));
    };
    if let Ok(index) = raw.parse::<u32>() {
        return if (index as usize) < roster.len() {
            Ok(SenderAdapter::Pinned(index))
        } else {
            Err(format!(
                "no graphics adapter at index {index}; this machine has {}",
                describe_roster(roster)
            ))
        };
    }
    match matches_for(raw, roster).as_slice() {
        [] => Err(format!(
            "no graphics adapter matching '{raw}'; this machine has {}",
            describe_roster(roster)
        )),
        [only] => Ok(SenderAdapter::Pinned(*only as u32)),
        several => Err(format!(
            "'{raw}' matches {} adapters: {}",
            several.len(),
            several
                .iter()
                .filter_map(|at| roster.get(*at).map(|name| format!("[{at}] {name}")))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Match the renderer's resolved adapter into the sender's roster.
///
/// Exact first, then containment either way: wgpu's DX12 backend reports the
/// DXGI description and the sender API reports the same field, so equality is
/// the expected case and containment is the tolerance for one of them carrying
/// a suffix the other does not.
fn follow_renderer(renderer_name: &str, roster: &[String]) -> SenderAdapter {
    if roster.is_empty() {
        return SenderAdapter::Default {
            reason: "this machine reports no Spout graphics adapters".to_owned(),
        };
    }
    let needle = renderer_name.to_lowercase();
    let exact: Vec<usize> = roster
        .iter()
        .enumerate()
        .filter(|(_, name)| name.to_lowercase() == needle)
        .map(|(at, _)| at)
        .collect();
    let hits = if exact.is_empty() {
        matches_for(renderer_name, roster)
    } else {
        exact
    };
    match hits.as_slice() {
        [only] => SenderAdapter::Pinned(*only as u32),
        [] if roster.len() == 1 => SenderAdapter::Pinned(0),
        [] => SenderAdapter::Default {
            reason: format!(
                "the renderer's adapter '{renderer_name}' matches none of {} \
                 — pass --gpu with the name of the GPU the receiver renders on",
                describe_roster(roster)
            ),
        },
        several => SenderAdapter::Default {
            reason: format!(
                "the renderer's adapter '{renderer_name}' matches {} of them \
                 — pass --gpu to say which",
                several.len()
            ),
        },
    }
}

/// Every roster position whose name contains `needle`, case-insensitively, in
/// either direction — so a shorter renderer name still finds a longer sender
/// name and the reverse.
fn matches_for(needle: &str, roster: &[String]) -> Vec<usize> {
    let needle = needle.to_lowercase();
    roster
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            let name = name.to_lowercase();
            name.contains(&needle) || needle.contains(&name)
        })
        .map(|(at, _)| at)
        .collect()
}

fn describe_roster(roster: &[String]) -> String {
    if roster.is_empty() {
        return "none".to_owned();
    }
    roster
        .iter()
        .enumerate()
        .map(|(at, name)| format!("[{at}] {name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HYBRID: [&str; 2] = [
        "AMD Radeon(TM) Graphics",
        "NVIDIA GeForce RTX 3080 Laptop GPU",
    ];

    fn hybrid() -> Vec<String> {
        HYBRID.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_flag_asks_the_renderer_for_high_performance() {
        assert_eq!(renderer_choice(None), AdapterChoice::HighPerformance);
    }

    #[test]
    fn a_bare_integer_is_an_index_and_anything_else_is_a_name() {
        assert_eq!(renderer_choice(Some("1")), AdapterChoice::Index(1));
        assert_eq!(
            renderer_choice(Some("RTX 3080")),
            AdapterChoice::Named("RTX 3080".to_owned())
        );
    }

    /// **The window's unflagged default is the stream's: high performance.**
    /// Both are live paths, and a live path on the power-saving GPU of a
    /// hybrid machine runs at a sixth of what the hardware allows (ADR-0246).
    /// The relation is pinned as agreement, so neither arm can quietly move
    /// off the other; the value itself is pinned too, so "agree" cannot be
    /// satisfied by both drifting to the plain default.
    #[test]
    fn the_window_and_the_stream_agree_when_unflagged() {
        assert_eq!(window_choice(None), AdapterChoice::HighPerformance);
        assert_eq!(renderer_choice(None), AdapterChoice::HighPerformance);
        assert_eq!(window_choice(None), renderer_choice(None));
    }

    /// **The flip is the standalone's, and the engine's default does not
    /// move.** `Renderer::new_from_surface_target` — the C ABI path the
    /// foobar shim builds its renderer through — takes
    /// `RendererOptions::default()`, so this is what keeps the plugin's
    /// window asking for exactly the adapter it asked for before (ADR-0246,
    /// ADR-0155).
    #[test]
    fn the_engine_default_the_foobar_shim_builds_on_is_unchanged() {
        use rlx_core::render::RendererOptions;
        assert_eq!(RendererOptions::default().adapter, AdapterChoice::Default);
        assert_ne!(
            RendererOptions::default().adapter,
            window_choice(None),
            "the standalone's preference leaked into the engine's default"
        );
    }

    /// **`--gpu` wins over `[output] gpu`, which wins over the default** — the
    /// precedence `--tier` has over `[quality] tier` — and the source says
    /// which carrier decided, so the startup line can name it.
    #[test]
    fn the_flag_beats_the_file_and_the_file_beats_the_default() {
        assert_eq!(
            resolve_window_choice(Some("RTX 3080"), Some("radeon")),
            (
                AdapterChoice::Named("RTX 3080".to_owned()),
                AdapterSource::Flag
            )
        );
        assert_eq!(
            resolve_window_choice(None, Some("radeon")),
            (
                AdapterChoice::Named("radeon".to_owned()),
                AdapterSource::Config
            )
        );
        // The file is read by the same name-or-index rule the flag is.
        assert_eq!(
            resolve_window_choice(None, Some("1")),
            (AdapterChoice::Index(1), AdapterSource::Config)
        );
        assert_eq!(
            resolve_window_choice(None, None),
            (window_choice(None), AdapterSource::Default)
        );
    }

    /// **A stored adapter falls back; a flagged one refuses — asserted in one
    /// place so the difference reads as a rule rather than an inconsistency.**
    /// The file outlives the machine state that made it valid, so a show that
    /// will not start is the wrong failure for it; the flag was typed for this
    /// run and keeps ADR-0155's hard refusal (ADR-0246).
    #[test]
    fn a_stored_adapter_falls_back_where_a_flagged_one_refuses() {
        let unresolved = [
            RenderError::NoSuchAdapter {
                requested: "Matrox".to_owned(),
                available: hybrid(),
            },
            RenderError::AmbiguousAdapter {
                requested: "NVIDIA".to_owned(),
                matched: hybrid(),
            },
            RenderError::AdapterCannotPresent {
                requested: "radeon".to_owned(),
                adapter: HYBRID[0].to_owned(),
            },
        ];
        for err in &unresolved {
            assert!(
                fallback_permitted(AdapterSource::Config, err),
                "a stored adapter must fall back on: {err}"
            );
            assert!(
                !fallback_permitted(AdapterSource::Flag, err),
                "a flagged adapter must refuse on: {err}"
            );
            assert!(
                !fallback_permitted(AdapterSource::Default, err),
                "the default has nothing to fall back to"
            );
        }
        // Only a *resolution* failure falls back: a device that enumerates and
        // then cannot be used is the same failure a default launch would hit.
        assert!(!fallback_permitted(
            AdapterSource::Config,
            &RenderError::UnsupportedSurface
        ));

        // The line names both what was asked for and that the default was
        // taken, so the fall-back is never silent.
        let note = fallback_note("Matrox");
        assert!(note.contains("\"Matrox\""), "{note}");
        assert!(note.contains("config.toml [output] gpu"), "{note}");
        assert!(note.contains(DEFAULT_WORD), "{note}");
        assert!(AdapterSource::Flag.note().contains("--gpu"));
        assert!(AdapterSource::Config.note().contains("config.toml"));
    }

    /// An explicit choice means the same thing on both paths — the operator
    /// types one string and it is read the same way whichever mode reads it.
    #[test]
    fn an_explicit_choice_is_spelled_the_same_on_both_paths() {
        for raw in ["1", "0", "RTX 3080", "radeon"] {
            assert_eq!(
                window_choice(Some(raw)),
                renderer_choice(Some(raw)),
                "`{raw}` is read differently by the window and the stream"
            );
        }
    }

    #[test]
    fn the_sender_follows_the_renderer_when_no_flag_is_given() {
        assert_eq!(
            sender_adapter(None, "NVIDIA GeForce RTX 3080 Laptop GPU", &hybrid()),
            Ok(SenderAdapter::Pinned(1))
        );
    }

    /// The failure Phase 3 measured: the sender on the integrated GPU is
    /// invisible to a receiver on the discrete one. Following the renderer must
    /// land on 1, never on 0.
    #[test]
    fn following_the_renderer_does_not_land_on_the_integrated_gpu() {
        let SenderAdapter::Pinned(index) =
            sender_adapter(None, "NVIDIA GeForce RTX 3080 Laptop GPU", &hybrid())
                .expect("the discrete GPU is in the roster")
        else {
            panic!("an exact name match must pin, not fall back");
        };
        assert_eq!(index, 1);
    }

    #[test]
    fn a_name_the_two_apis_spell_differently_still_matches() {
        // wgpu reporting a bare model where the sender carries the full string.
        let roster = hybrid();
        assert_eq!(
            sender_adapter(None, "NVIDIA GeForce RTX 3080", &roster),
            Ok(SenderAdapter::Pinned(1))
        );
    }

    #[test]
    fn an_unmatched_renderer_name_falls_back_and_says_why() {
        let SenderAdapter::Default { reason } =
            sender_adapter(None, "Intel Arc A770", &hybrid()).expect("a fall-back is not an error")
        else {
            panic!("an unmatched name must fall back, not pin");
        };
        assert!(reason.contains("Intel Arc A770"), "{reason}");
        assert!(reason.contains("--gpu"), "{reason}");
    }

    /// A single-adapter machine has one answer and the match cannot be wrong,
    /// so it pins rather than warning about a choice that does not exist.
    #[test]
    fn one_adapter_pins_even_when_the_names_disagree() {
        let roster = vec!["Some Vendor Graphics".to_owned()];
        assert_eq!(
            sender_adapter(None, "a name that matches nothing", &roster),
            Ok(SenderAdapter::Pinned(0))
        );
    }

    #[test]
    fn an_explicit_name_pins_the_sender() {
        assert_eq!(
            sender_adapter(Some("rtx 3080"), "irrelevant", &hybrid()),
            Ok(SenderAdapter::Pinned(1))
        );
        assert_eq!(
            sender_adapter(Some("radeon"), "irrelevant", &hybrid()),
            Ok(SenderAdapter::Pinned(0))
        );
    }

    #[test]
    fn an_explicit_index_pins_the_sender_and_is_range_checked() {
        assert_eq!(
            sender_adapter(Some("0"), "irrelevant", &hybrid()),
            Ok(SenderAdapter::Pinned(0))
        );
        let err = sender_adapter(Some("7"), "irrelevant", &hybrid())
            .expect_err("index 7 is not on a two-adapter machine");
        assert!(err.contains("index 7"), "{err}");
        assert!(err.contains("AMD"), "the error lists the roster: {err}");
        assert!(err.contains("NVIDIA"), "the error lists the roster: {err}");
    }

    #[test]
    fn an_unresolvable_name_names_the_roster_rather_than_an_index() {
        let err = sender_adapter(Some("Matrox"), "irrelevant", &hybrid())
            .expect_err("no Matrox on this machine");
        assert!(err.contains("Matrox"), "{err}");
        assert!(err.contains("[0] AMD Radeon(TM) Graphics"), "{err}");
        assert!(
            err.contains("[1] NVIDIA GeForce RTX 3080 Laptop GPU"),
            "{err}"
        );
    }

    /// Two adapters sharing a substring cannot be separated by name, and an
    /// arbitrary pick would be the silent wrong-GPU failure again.
    #[test]
    fn an_ambiguous_name_is_an_error_and_not_a_pick() {
        let roster = vec![
            "NVIDIA GeForce RTX 3080".to_owned(),
            "NVIDIA GeForce RTX 4090".to_owned(),
        ];
        let err = sender_adapter(Some("NVIDIA"), "irrelevant", &roster)
            .expect_err("'NVIDIA' cannot separate two NVIDIA adapters");
        assert!(err.contains("matches 2 adapters"), "{err}");
        assert!(err.contains("[0]") && err.contains("[1]"), "{err}");
    }
}
