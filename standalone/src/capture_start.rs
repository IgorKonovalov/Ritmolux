//! Starting, losing and reopening the capture stream.
//!
//! The three `start_capture` arms are the whole of the platform branch the
//! shell carries: Windows opens WASAPI, macOS opens ScreenCaptureKit, and every
//! other target reports [`CaptureVerdict::Unsupported`] rather than failing to
//! build. Each returns the same [`CaptureStart`], so the caller never asks which
//! platform it is on.
//!
//! [`RecoveryPolicy`] is the other half: a stream that dies is reopened a
//! bounded number of times and then let go, once, rather than retried forever.

use rlx_core::audio::{AudioFormat, SampleConsumer};

use crate::capture_verdict::CaptureVerdict;
#[cfg(windows)]
use crate::capture_win;
use crate::config;

/// Narrow alias so the non-Windows build, which has no capture, compiles the
/// same struct shape.
pub(crate) mod capture_handle {
    #[cfg(target_os = "macos")]
    pub type Handle = crate::capture_mac::CaptureHandle;
    #[cfg(windows)]
    pub type Handle = crate::capture_win::CaptureHandle;
    #[cfg(not(any(windows, target_os = "macos")))]
    pub type Handle = ();
}

/// What one call to `start_capture` produced: the handle and consumer when it
/// worked, the format the analyzer is built on either way, and — the point of
/// Plan 0083 — the [`CaptureVerdict`] that says which of those two happened.
pub(crate) struct CaptureStart {
    pub(crate) handle: Option<capture_handle::Handle>,
    pub(crate) consumer: Option<SampleConsumer>,
    pub(crate) format: AudioFormat,
    pub(crate) verdict: CaptureVerdict,
    /// The endpoint the stream actually opened, by friendly name — the resolved
    /// one, so a selection that degraded names what is running. `None` when the
    /// start failed, and on a platform whose capture path picks no endpoint.
    pub(crate) endpoint: Option<String>,
}

/// Whether a capture swap writes its selection back to `config.toml`.
///
/// The distinction is who asked. A settings row is an operator choosing an
/// input, and the file records it like every other row. A recovery is the shell
/// keeping the show alive on whatever endpoint is left, which is not a choice
/// and must not overwrite one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Persist {
    Yes,
    No,
}

/// How many times a lost input is reopened before the shell stops trying.
///
/// Each attempt is a synchronous WASAPI activation on the render thread — a
/// visible hitch — and what is being guarded against is one of those per frame
/// against an audio subsystem that is not coming back. Three covers the case the
/// recovery exists for, a device that returns within a moment of a driver reset,
/// without letting a permanently removed interface stutter the show for longer
/// than the silence would have.
pub(crate) const INPUT_RECOVERY_ATTEMPTS: u32 = 3;

/// How long a recovered input must keep delivering before it is judged settled
/// and its retry budget restored.
///
/// Without a window a single live frame restores the budget, so a stream that
/// opens and dies immediately — a flapping USB interface, a driver resetting in
/// a loop — gets a fresh three attempts every cycle and reopens for the rest of
/// the show. That is the blocking device activation per frame
/// [`INPUT_RECOVERY_ATTEMPTS`] exists to bound, reached by a different road. A
/// stream that survives this long is delivering rather than merely constructed:
/// an invalidated endpoint reports itself on the first packet call after start,
/// well inside it.
///
/// **In seconds, because a frame count is a different guarantee on every
/// display.** 0.36 s is what the 60-frame window it replaces bought on the
/// 165 Hz box it was written on — the fast end, where a reader assuming "about
/// a second" was wrong by nearly 3x. The same 60 frames spanned ~2 s at 30 Hz
/// and ~250 ms at 240 Hz, so the flap it guards against was bounded on one
/// display and unbounded on another.
///
/// The cost is that a genuine second unplug within the window inherits the first
/// incident's remaining budget instead of a full one — for a *recovery's* own
/// reopen. An operator picking an input resets the policy instead
/// ([`RecoveryPolicy::on_restart`]), because a human judging that the situation
/// changed is not the flap this window guards against.
pub(crate) const INPUT_RECOVERY_SETTLE_SECS: f32 = 0.36;

/// What the shell should do about the capture stream this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recovery {
    /// Nothing: the stream is alive, or the bound is already spent and said so.
    Hold,
    /// Reopen on the mode's default endpoint; the payload is the 1-based attempt.
    Reopen(u32),
    /// The bound is spent. Emitted **once**, on the frame it runs out, so the
    /// verdict is rewritten and the operator told exactly one time.
    GiveUp,
}

/// The bounded-retry rule, as a value.
///
/// Pure and WASAPI-free: the shell hands it whether the input is lost and does
/// what it says, which is what makes the bound assertable without an audio
/// device to unplug.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct RecoveryPolicy {
    pub(crate) attempts: u32,
    pub(crate) announced: bool,
    /// Seconds of unbroken delivery since the last loss, counted only while a
    /// budget is actually spent — an input that has never been lost has nothing
    /// to restore and never enters the window.
    pub(crate) settled: f32,
}

impl RecoveryPolicy {
    /// Fold a capture restart into the policy: an operator's choice begins a new
    /// incident, a recovery's own reopen continues the one in progress.
    ///
    /// Without this an operator who picks a working input after a give-up leaves
    /// `attempts` at the bound and `announced` set, so a stream that dies inside
    /// the settle window takes the `Hold` arm — no reopen, which is intended, and
    /// **no token rewrite**, which is not: the `capture` column and the F3
    /// overlay go on reading `live` while nothing is delivering.
    ///
    /// Restoring the retry budget is a deliberate consequence rather than a side
    /// effect. It is the exact case [`INPUT_RECOVERY_SETTLE_SECS`] gives up, and
    /// giving it up is only defensible for the flap that constant guards
    /// against, not for a human who has just judged that the situation changed.
    pub(crate) fn on_restart(&mut self, persist: Persist) {
        if persist == Persist::Yes {
            *self = Self::default();
        }
    }

    /// Advance one frame, given the seconds it covered.
    ///
    /// `dt` is the shell's own frame time, already clamped to [`MAX_DT`], so a
    /// stalled or resumed window cannot hand the accumulator a jump large enough
    /// to settle a stream that never delivered.
    pub(crate) fn poll(&mut self, lost: bool, dt: f32) -> Recovery {
        if !lost {
            if self.attempts == 0 {
                return Recovery::Hold;
            }
            // A device lost twice in one show should get a real second chance
            // rather than the remainder of the first — but only once the stream
            // has proved it is delivering, or a flap would restore the budget
            // faster than the bound can spend it.
            self.settled += dt;
            if self.settled >= INPUT_RECOVERY_SETTLE_SECS {
                *self = Self::default();
            }
            return Recovery::Hold;
        }
        self.settled = 0.0;
        if self.attempts < INPUT_RECOVERY_ATTEMPTS {
            self.attempts += 1;
            return Recovery::Reopen(self.attempts);
        }
        if self.announced {
            return Recovery::Hold;
        }
        self.announced = true;
        Recovery::GiveUp
    }
}

/// Whether the running capture stream has reported itself dead.
///
/// Only the Windows path reports it; elsewhere a stream is either running or was
/// never started, and there is nothing to observe.
#[cfg(windows)]
pub(crate) fn capture_lost(handle: Option<&capture_handle::Handle>) -> bool {
    handle.is_some_and(capture_win::CaptureHandle::lost)
}

#[cfg(not(windows))]
pub(crate) fn capture_lost(_handle: Option<&capture_handle::Handle>) -> bool {
    false
}

/// The short name of this platform's capture path, as the verdict token carries
/// it. One constant so the live, failed and lost tokens of a run cannot name
/// three different backends.
#[cfg(windows)]
pub(crate) const CAPTURE_BACKEND: &str = "WASAPI";
#[cfg(target_os = "macos")]
pub(crate) const CAPTURE_BACKEND: &str = "SCK";
#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) const CAPTURE_BACKEND: &str = "none";

/// The device name that means "the mode's default endpoint" — the value
/// `config.toml` ships with, the word `pick_device` treats as "no name to
/// match", and the leading entry of the settings menu's endpoint roster.
pub(crate) const DEFAULT_ENDPOINT: &str = "default";

/// The format both platform arms fall back to when capture fails, so the analyzer
/// has something valid to start on. **The verdict never reports it** — a log
/// stating a format nothing is delivering is worse than no log.
pub(crate) const FALLBACK_FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// The roster position of the endpoint a stream actually opened.
///
/// Pure, so the one property that matters is assertable without a window: the
/// row is positioned by what capture **reports running** (`running`), never by
/// re-matching the configured name against the roster. The configured name is
/// consulted for exactly one thing — whether it is the `default` word, which is
/// the roster's leading slot whatever endpoint it resolved to — and that test is
/// the same one `start_capture` uses to decide there is no name to match.
///
/// A name that matched no endpoint degraded to the default endpoint inside the
/// capture layer, so `running` names that endpoint and the row does too, rather
/// than echoing a request nothing honoured.
pub(crate) fn device_row_index(
    roster: &[String],
    configured: &str,
    running: Option<&str>,
) -> usize {
    if configured.trim().is_empty() || configured.eq_ignore_ascii_case(DEFAULT_ENDPOINT) {
        return 0;
    }
    let running = running.unwrap_or_default();
    roster.iter().position(|name| name == running).unwrap_or(0)
}

/// The capture layer's mode for a config mode — the one place the two enums
/// meet, so `core`'s source-agnostic split does not turn into a match repeated
/// at every call site.
#[cfg(windows)]
pub(crate) fn capture_mode(mode: config::InputMode) -> capture_win::CaptureMode {
    match mode {
        config::InputMode::Loopback => capture_win::CaptureMode::Loopback,
        config::InputMode::LineIn => capture_win::CaptureMode::LineIn,
    }
}

#[cfg(windows)]
pub(crate) fn start_capture(input: &config::Input) -> CaptureStart {
    let selector = capture_win::CaptureSelector {
        mode: capture_mode(input.mode),
        // "default" (or empty) means the mode's default endpoint — no name match.
        device: (!input.device.trim().is_empty()
            && !input.device.eq_ignore_ascii_case(DEFAULT_ENDPOINT))
        .then(|| input.device.clone()),
    };
    match capture_win::start(&selector) {
        Ok((handle, consumer)) => {
            let format = handle.format();
            let endpoint = handle.device().to_owned();
            let verdict = CaptureVerdict::live(CAPTURE_BACKEND, format, &endpoint);
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict,
                endpoint: Some(endpoint),
            }
        }
        Err(err) => {
            // Stays: it costs nothing and is still the fastest read for anyone
            // already at a terminal. The verdict is for everyone who is not.
            eprintln!("audio capture unavailable ({err}); rendering without audio");
            CaptureStart {
                handle: None,
                consumer: None,
                format: FALLBACK_FORMAT,
                verdict: CaptureVerdict::failed(CAPTURE_BACKEND, err),
                endpoint: None,
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn start_capture(_input: &config::Input) -> CaptureStart {
    match capture_mac::start() {
        Ok((handle, consumer)) => {
            let format = handle.format();
            // ScreenCaptureKit taps the system mix rather than an endpoint the
            // caller picks, so there is no device name to report.
            let verdict = CaptureVerdict::live(CAPTURE_BACKEND, format, "system audio");
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict,
                // Not an endpoint anything can select, so it positions no row.
                endpoint: None,
            }
        }
        Err(err) => {
            eprintln!("ScreenCaptureKit capture unavailable ({err}); rendering without audio");
            CaptureStart {
                handle: None,
                consumer: None,
                format: FALLBACK_FORMAT,
                verdict: CaptureVerdict::failed(CAPTURE_BACKEND, err),
                endpoint: None,
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn start_capture(_input: &config::Input) -> CaptureStart {
    // No capture path on this platform; render silence-driven visuals.
    CaptureStart {
        handle: None,
        consumer: None,
        format: FALLBACK_FORMAT,
        verdict: CaptureVerdict::Unsupported,
        endpoint: None,
    }
}

/// Print the enumerable audio devices (the `--list-devices` aid). Windows-first
/// per ADR-0001; other platforms note that device selection isn't wired there.
#[cfg(windows)]
pub(crate) fn list_devices_and_exit() {
    if let Err(err) = capture_win::list_devices() {
        eprintln!("could not list audio devices: {err}");
    }
}

#[cfg(not(windows))]
pub(crate) fn list_devices_and_exit() {
    eprintln!("--list-devices is Windows-only (Plan 0009 Phase 2)");
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        INPUT_RECOVERY_ATTEMPTS, INPUT_RECOVERY_SETTLE_SECS, Persist, Recovery, RecoveryPolicy,
        device_row_index,
    };

    /// One frame at 60 Hz, for the cases where the *rate* is not what is under
    /// test — the bound, the flap and the give-up latch all count events rather
    /// than seconds, so any frame time exercises them the same way.
    const A_FRAME: f32 = 1.0 / 60.0;

    /// **A live input costs nothing and never reopens.** The policy runs every
    /// frame of every show, so the overwhelmingly common answer has to be `Hold`.
    #[test]
    fn a_live_input_is_never_reopened() {
        let mut policy = RecoveryPolicy::default();
        for _ in 0..1000 {
            assert_eq!(policy.poll(false, A_FRAME), Recovery::Hold);
        }
        assert_eq!(policy, RecoveryPolicy::default(), "a live input kept state");
    }

    /// **The retry is bounded and then silent.** An unbounded retry is a
    /// blocking device activation per frame against a subsystem that is not
    /// coming back, which stutters the show worse than the silence does — and
    /// `GiveUp` arrives exactly once, so neither the operator nor the verdict is
    /// told the same thing on every subsequent frame.
    #[test]
    fn a_lost_input_reopens_a_bounded_number_of_times_then_gives_up_once() {
        let mut policy = RecoveryPolicy::default();
        for attempt in 1..=INPUT_RECOVERY_ATTEMPTS {
            assert_eq!(
                policy.poll(true, A_FRAME),
                Recovery::Reopen(attempt),
                "attempt {attempt} did not reopen"
            );
        }
        assert_eq!(policy.poll(true, A_FRAME), Recovery::GiveUp);
        for _ in 0..500 {
            assert_eq!(
                policy.poll(true, A_FRAME),
                Recovery::Hold,
                "the policy kept talking after it gave up"
            );
        }
    }

    /// **A recovered input gets its whole budget back — once it has settled.**
    /// An interface unplugged twice in one show is two incidents, not one long
    /// one, so the second must not inherit the remainder of the first. What the
    /// window buys is that "recovered" means a stream that kept delivering, not
    /// one that merely got constructed; it is asserted from both sides, because
    /// an off-by-one here is the difference between this rule and the one below
    /// it.
    ///
    /// **Swept over three refresh rates**, which is the property a frame count
    /// could not state: 30 Hz and 240 Hz are the ends where 60 frames meant
    /// about 2 s and about 250 ms, and the window has to be the same length of
    /// time at both.
    #[test]
    fn the_settle_window_is_one_duration_on_every_display() {
        // The two ends where the frame-count version gave different guarantees,
        // plus the 165 Hz box whose 60 frames the constant is derived from.
        for hz in [30.0_f32, 165.0, 240.0] {
            let dt = 1.0 / hz;
            let mut policy = RecoveryPolicy::default();
            assert_eq!(policy.poll(true, dt), Recovery::Reopen(1));
            assert_eq!(policy.poll(true, dt), Recovery::Reopen(2));

            // Up to the last frame that still lands inside the window, the
            // budget is spent.
            let mut elapsed = 0.0_f32;
            while elapsed + dt < INPUT_RECOVERY_SETTLE_SECS {
                assert_eq!(policy.poll(false, dt), Recovery::Hold);
                elapsed += dt;
            }
            assert_ne!(
                policy,
                RecoveryPolicy::default(),
                "at {hz} Hz the budget came back after {elapsed} s, inside the window"
            );

            // The frame that completes it restores everything.
            assert_eq!(policy.poll(false, dt), Recovery::Hold);
            elapsed += dt;
            assert_eq!(
                policy,
                RecoveryPolicy::default(),
                "at {hz} Hz the budget had not come back after {elapsed} s"
            );

            // The point of the change: the window closes at the same *time* on
            // every display, to within the one frame no policy can subdivide.
            assert!(
                elapsed >= INPUT_RECOVERY_SETTLE_SECS && elapsed < INPUT_RECOVERY_SETTLE_SECS + dt,
                "at {hz} Hz the window closed at {elapsed} s, which is not within one frame of the window"
            );

            for attempt in 1..=INPUT_RECOVERY_ATTEMPTS {
                assert_eq!(policy.poll(true, dt), Recovery::Reopen(attempt));
            }
            assert_eq!(policy.poll(true, dt), Recovery::GiveUp);
        }
    }

    /// **A flapping endpoint cannot outrun the bound.** A stream that opens and
    /// dies on the very next frame is the case that defeats a budget restored by
    /// a single live frame: it would hand back three attempts every cycle and
    /// reopen for the rest of the show, which is the blocking device activation
    /// per frame the bound exists to prevent, reached by a different road than
    /// a device that never comes back at all.
    #[test]
    fn a_stream_that_dies_as_fast_as_it_opens_still_gives_up() {
        let mut policy = RecoveryPolicy::default();
        let mut reopens = 0;
        let mut gave_up = 0;
        for _ in 0..10_000 {
            match policy.poll(true, A_FRAME) {
                Recovery::Reopen(_) => {
                    reopens += 1;
                    // The reopen "worked" — for exactly one frame.
                    assert_eq!(policy.poll(false, A_FRAME), Recovery::Hold);
                }
                Recovery::GiveUp => gave_up += 1,
                Recovery::Hold => {}
            }
        }
        assert_eq!(
            reopens, INPUT_RECOVERY_ATTEMPTS,
            "a flapping endpoint reopened past the bound"
        );
        assert_eq!(gave_up, 1, "the give-up notice did not arrive exactly once");
    }

    /// **The device row is positioned by what capture reports running**, never
    /// by re-matching the configured name against the roster.
    ///
    /// The third case is the one that motivates the rule and the only one a
    /// second matcher gets wrong: `pick_device` matches exact across *every*
    /// endpoint before it tries substring across every endpoint, so an exact
    /// name that is also a substring of an earlier entry opens the later one —
    /// while a per-element exact-or-substring pass stops at the earlier entry
    /// and highlights a row the stream is not on. Neither name in the pair on
    /// the development box is a substring of the other, which is why nothing
    /// there can tell the two rules apart.
    #[test]
    fn the_device_row_follows_the_endpoint_that_is_running() {
        let roster: Vec<String> = ["default", "Headphones (2- USB Audio)", "Headphones"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        // The `default` word is the leading slot whatever it resolved to — the
        // row and `config.toml` name the same thing.
        assert_eq!(
            device_row_index(&roster, "default", Some("Headphones (2- USB Audio)")),
            0
        );
        assert_eq!(device_row_index(&roster, "  ", Some("Headphones")), 0);

        // The substring trap: `pick_device` opened the exact match at index 2.
        assert_eq!(
            device_row_index(&roster, "Headphones", Some("Headphones")),
            2
        );

        // A name that matched nothing degraded inside the capture layer, and the
        // row names the endpoint actually running rather than the request.
        assert_eq!(
            device_row_index(&roster, "ZOOM AMS-22", Some("Headphones (2- USB Audio)")),
            1
        );

        // Nothing running, and a roster that does not contain what is: index 0
        // rather than a panic or an out-of-range row.
        assert_eq!(device_row_index(&roster, "Headphones", None), 0);
        assert_eq!(device_row_index(&roster, "Headphones", Some("Gone")), 0);
        assert_eq!(device_row_index(&[], "Headphones", Some("Headphones")), 0);
    }

    /// A policy that has spent its budget and announced the give-up — the state
    /// an operator reaches for the `S` menu in.
    fn given_up() -> RecoveryPolicy {
        let mut policy = RecoveryPolicy::default();
        for _ in 0..INPUT_RECOVERY_ATTEMPTS {
            policy.poll(true, A_FRAME);
        }
        assert_eq!(policy.poll(true, A_FRAME), Recovery::GiveUp);
        policy
    }

    /// **An operator's choice is a new incident.** After a give-up, a swap to a
    /// working input, and a death inside the settle window, the spent latch
    /// returns `Hold` — and `Hold` writes no token, so the `capture` column and
    /// the F3 overlay would go on reading `live` about a stream delivering
    /// nothing. The reset is what makes that loss reach `GiveUp` again and
    /// rewrite the verdict to `lost`.
    #[test]
    fn an_operator_swap_makes_the_next_loss_write_its_own_verdict() {
        let mut inherited = given_up();
        assert_eq!(
            inherited.poll(true, A_FRAME),
            Recovery::Hold,
            "a spent policy is silent, which is the failure"
        );

        let mut reset = given_up();
        reset.on_restart(Persist::Yes);
        for attempt in 1..=INPUT_RECOVERY_ATTEMPTS {
            assert_eq!(reset.poll(true, A_FRAME), Recovery::Reopen(attempt));
        }
        assert_eq!(
            reset.poll(true, A_FRAME),
            Recovery::GiveUp,
            "the loss after an operator swap wrote no verdict"
        );
    }

    /// **A recovery's own reopen still inherits.** The bound exists to stop a
    /// flapping endpoint handing itself a fresh budget every cycle, and nothing
    /// about a reopen the shell asked for itself says the situation changed.
    #[test]
    fn a_recoverys_own_restart_does_not_reset_the_policy() {
        let mut policy = given_up();
        policy.on_restart(Persist::No);
        assert_eq!(policy.poll(true, A_FRAME), Recovery::Hold);
        assert_ne!(policy, RecoveryPolicy::default());
    }
}
