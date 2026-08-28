//! What happened when the shell tried to start capture, as a **value** (Plan 0083).
//!
//! The degradation on a failed capture is right and stays — the app renders its
//! idle animation without audio. What was wrong is that the *reason* existed only
//! on stderr, which a Finder or Explorer launch discards: a remote tester's log
//! could prove capture never delivered a sample and could not say why.
//!
//! So the verdict is decided on the render/UI thread, rendered into a short token
//! there, and then only *borrowed* — by the `diagnostics.log` row builder (which
//! runs every frame) and by the F3 overlay. Both surfaces read the same stored
//! string, so they cannot disagree about what is running.
//!
//! It is **current state, not a startup fact** (ADR-0142). The input can be
//! swapped from the settings menu and can be lost and recovered underneath the
//! app, so the token is re-rendered on every swap and both surfaces answer *what
//! is capture listening to now*. What does not change is that it is rendered in
//! one place and stored: two surfaces formatting a verdict apiece is how a log
//! and an overlay come to disagree.
//!
//! Nothing here touches `core/`: capture is a shell concern by ADR-0001.

use std::fmt;

use lmv_core::audio::AudioFormat;

/// What `start_capture` concluded.
///
/// `backend` is the short static name of the platform capture path — `"WASAPI"`,
/// `"SCK"` — passed in at the call site rather than held here as a constant, so a
/// name only exists on the platform that has that path. It is in the token so a
/// log says *which* route was tried, not merely that one failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureVerdict {
    /// Capture started, `format` is what it negotiated, and `endpoint` is the
    /// friendly name of the device it actually opened.
    ///
    /// The endpoint is the *resolved* one, not the configured one: a selection
    /// that named an absent interface degrades to the default, and a log that
    /// echoed the request would say the opposite of what is happening.
    Live {
        backend: &'static str,
        format: AudioFormat,
        endpoint: String,
    },
    /// A stream that *was* delivering reported itself gone, and the bounded
    /// recovery did not get another one.
    ///
    /// Distinct from [`Failed`](Self::Failed), which is a start that never
    /// worked: this run had audio and lost it, and a log showing only the last
    /// reopen's error would not say that.
    Lost {
        backend: &'static str,
        attempts: u32,
    },
    /// The platform capture path failed; `reason` is the error's `Display`.
    ///
    /// **Deliberately carries no format.** Both platform arms fall back to a
    /// hardcoded 48 kHz stereo so the analyzer has something valid to start on,
    /// and reporting that fallback here would have the log state a format nothing
    /// is delivering.
    Failed {
        backend: &'static str,
        reason: String,
    },
    /// Built for a platform with no capture path at all.
    ///
    /// Constructed only by the `not(any(windows, target_os = "macos"))` arm of
    /// `start_capture`, so on either shipping platform it is dead by
    /// construction — which is the point: the third arm has to render as
    /// something, and "nothing was ever tried" is not a success.
    #[cfg_attr(
        any(windows, target_os = "macos"),
        allow(
            dead_code,
            reason = "only the no-capture-path arm constructs it; that arm is cfg'd out here"
        )
    )]
    Unsupported,
}

impl CaptureVerdict {
    /// A live verdict, with the endpoint name sanitized on the way in — a
    /// friendly name comes from a driver and is not under our control, and the
    /// log is one tab-separated row per line.
    pub fn live(backend: &'static str, format: AudioFormat, endpoint: &str) -> Self {
        Self::Live {
            backend,
            format,
            endpoint: sanitize(endpoint),
        }
    }

    /// A failed verdict from a platform error, sanitized on the way in.
    pub fn failed(backend: &'static str, err: impl fmt::Display) -> Self {
        Self::Failed {
            backend,
            reason: sanitize(&err.to_string()),
        }
    }

    /// The one-line token both durable artifacts carry — e.g. `live SCK 48000/2`,
    /// `failed SCK <reason>`, `unsupported`.
    ///
    /// Built **once**, at startup, and stored: `diaglog`'s row builder runs every
    /// frame and borrows this rather than formatting it again.
    pub fn token(&self) -> String {
        match self {
            Self::Live {
                backend,
                format,
                endpoint,
            } => format!(
                "live {backend} {}/{} {endpoint}",
                format.sample_rate, format.channels
            ),
            Self::Lost { backend, attempts } => {
                format!("lost {backend} not recovered in {attempts} attempts")
            }
            Self::Failed { backend, reason } => format!("failed {backend} {reason}"),
            Self::Unsupported => "unsupported".to_owned(),
        }
    }
}

/// Strip whatever a platform error's `Display` might contain that the artifacts
/// cannot carry: `diagnostics.log` is tab-separated with one row per line, and an
/// OS error message is not under our control. Tabs and newlines become spaces,
/// runs of whitespace collapse, and the result is trimmed — so a hostile message
/// can widen the field's text but never the row.
fn sanitize(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());
    let mut pending_space = false;
    for c in reason.chars() {
        if c.is_whitespace() || c.is_control() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c);
    }
    if out.is_empty() {
        // An error whose whole message was whitespace still has to say something,
        // or the row would carry a `failed SCK ` that reads like a truncation.
        out.push_str("(no message)");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMAT: AudioFormat = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    const SCK: &str = "SCK";
    const WASAPI: &str = "WASAPI";

    /// **Every arm renders something, and no two arms render the same thing.**
    /// A future arm that forgets to set its verdict would otherwise render as a
    /// success, which is the exact failure this type exists to prevent.
    #[test]
    fn the_three_verdicts_are_distinguishable_non_empty_strings() {
        let live = CaptureVerdict::live(SCK, FORMAT, "MacBook Pro Speakers").token();
        let failed = CaptureVerdict::failed(SCK, "screen recording permission denied").token();
        let lost = CaptureVerdict::Lost {
            backend: SCK,
            attempts: 3,
        }
        .token();
        let unsupported = CaptureVerdict::Unsupported.token();

        for token in [&live, &failed, &lost, &unsupported] {
            assert!(!token.is_empty(), "an empty token says nothing: {token:?}");
        }
        assert_ne!(live, failed, "a failed capture reads as a live one");
        assert_ne!(live, unsupported);
        assert_ne!(failed, unsupported);
        // The one this distinction is for: a run that had audio and lost it must
        // not read as a run that never started.
        assert_ne!(
            lost, failed,
            "a lost input reads as a start that never worked"
        );
        assert_ne!(lost, live);
        assert_ne!(lost, unsupported);

        // The live token carries the negotiated format *and* the endpoint, which
        // is what a tester's "what is it listening to" question turns on.
        assert_eq!(live, "live SCK 48000/2 MacBook Pro Speakers");
        assert!(
            lost.contains('3'),
            "the lost token drops how hard it tried: {lost:?}"
        );
        assert!(
            failed.starts_with("failed SCK ") && failed.contains("permission denied"),
            "the failed token drops the reason: {failed:?}"
        );
    }

    /// **The row survives a hostile error message.** A deliberately nasty string
    /// rather than a real platform error: a real one that happens to be clean
    /// proves nothing about the sanitizer.
    #[test]
    fn a_reason_with_tabs_and_newlines_cannot_break_a_row() {
        let hostile = "  start failed:\tcode -3801\r\n\tat SCStream\n\n";
        let token = CaptureVerdict::failed(SCK, hostile).token();
        assert!(
            !token.contains('\t') && !token.contains('\n') && !token.contains('\r'),
            "the token can corrupt a tab-separated row: {token:?}"
        );
        assert_eq!(token, "failed SCK start failed: code -3801 at SCStream");
    }

    /// **An endpoint name is driver-supplied**, so it goes through the same
    /// sanitizer the error reason does — otherwise a device whose friendly name
    /// carried a tab would widen the row rather than the field.
    #[test]
    fn an_endpoint_name_cannot_break_a_row_either() {
        let token = CaptureVerdict::live(WASAPI, FORMAT, "Line\t(ZOOM\nAMS-22)").token();
        assert!(
            !token.contains('\t') && !token.contains('\n'),
            "the endpoint can corrupt a tab-separated row: {token:?}"
        );
        assert_eq!(token, "live WASAPI 48000/2 Line (ZOOM AMS-22)");
    }

    /// An error that renders to nothing still names a failure, rather than
    /// trailing off into a field that reads like a truncated write.
    #[test]
    fn an_empty_reason_still_says_something() {
        let token = CaptureVerdict::failed(WASAPI, "   ").token();
        assert_eq!(token, "failed WASAPI (no message)");
    }
}
