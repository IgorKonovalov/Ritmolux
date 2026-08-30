//! `lmv --stream`: the headless live video source (ADR-0125).
//!
//! No window and no swapchain. Loopback audio drives the analyzer, the renderer
//! draws through the same `draw_frame` the window presents through, the frame
//! tap reads each frame back, and a Spout sender publishes it. TouchDesigner
//! picks it up with a `Syphon Spout In` TOP on the same machine.
//!
//! **Pacing is deadline-based, not sleep-per-frame.** Frame `n` is due at
//! `n * period` measured from the start of the run, so a frame that overruns
//! costs only itself: the next one is due at its original time and the run does
//! not drift away from the wall clock. A run that falls behind emits fewer,
//! correctly-timed frames rather than a picture running slow against the music,
//! which is the property `Renderer::render_tapped`'s per-call `dt` exists for.
//!
//! Everything above [`run`] is a pure function of its arguments and is unit
//! tested with no GPU, no audio device and no Spout SDK.

// With no Spout sink compiled there is no `run` to call the pure half, and
// dead-code analysis does not see the unit tests that do. These are the mode's
// argument and pacing contract; they stay compiled and checked on every
// configuration rather than being cfg'd out alongside the sink, so a change to
// them is caught by an ordinary featureless build.
#![cfg_attr(
    not(all(feature = "spout", windows)),
    allow(
        dead_code,
        reason = "the pure half's only non-test caller is the Spout-gated `run`"
    )
)]

use std::time::Duration;

/// The sender name a receiver lists, unless `--sender` overrides it. Not
/// necessarily the name that gets registered: a stale registration from a
/// crashed run makes `SetSenderName` increment, which is why the mode prints
/// what it actually got.
pub const DEFAULT_SENDER: &str = "lmv";

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;
const DEFAULT_FPS: u32 = 60;

/// Upper bound on a requested frame rate. Not a capability claim - it rejects a
/// typo (`--fps 6000`) before it becomes a busy loop.
const MAX_FPS: u32 = 240;

/// Upper bound on a requested dimension, matching the largest target this
/// engine is exercised at with headroom. Rejects a transposed or mistyped size
/// before a multi-gigabyte allocation is attempted.
const MAX_DIMENSION: u32 = 7680;

/// What `--stream` was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    /// Published frame width in pixels.
    pub width: u32,
    /// Published frame height in pixels.
    pub height: u32,
    /// Target frame rate.
    pub fps: u32,
    /// The operator's GPU name or index, resolved against **both** adapter
    /// rosters (ADR-0146). `None` leaves the renderer on `HighPerformance` and
    /// the sender following it by name.
    pub gpu: Option<String>,
    /// The sender name to claim.
    pub sender: String,
    /// Stop after this many frames, for a bounded measured run. `None` runs
    /// until Ctrl-C.
    pub frames: Option<u64>,
    /// Hold this preset for the whole run and rotate nothing. `None` rotates on
    /// the director's dwell timer.
    pub preset: Option<String>,
}

impl Default for StreamRequest {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            fps: DEFAULT_FPS,
            gpu: None,
            sender: DEFAULT_SENDER.to_owned(),
            frames: None,
            preset: None,
        }
    }
}

impl StreamRequest {
    /// One frame's worth of wall-clock time.
    pub fn period(&self) -> Duration {
        Duration::from_nanos(1_000_000_000 / u64::from(self.fps.max(1)))
    }
}

/// Parse `--stream` and its arguments out of a command line.
///
/// `Ok(None)` means `--stream` was not asked for and the app should start
/// normally; every other flag here is only read when it was.
pub fn parse(args: &[String]) -> Result<Option<StreamRequest>, String> {
    if !args.iter().any(|arg| arg == "--stream") {
        return Ok(None);
    }
    let mut request = StreamRequest::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--stream" => {}
            "--size" => {
                let raw = rest.next().ok_or("--size: expected WIDTHxHEIGHT")?;
                let (width, height) = parse_size(raw)?;
                request.width = width;
                request.height = height;
            }
            "--fps" => {
                let raw = rest.next().ok_or("--fps: expected a frame rate")?;
                request.fps = parse_fps(raw)?;
            }
            "--gpu" => {
                let raw = rest
                    .next()
                    .ok_or("--gpu: expected an adapter name or index")?;
                request.gpu = Some(raw.clone());
            }
            "--sender" => {
                let raw = rest.next().ok_or("--sender: expected a name")?;
                if raw.trim().is_empty() {
                    return Err("--sender: the name cannot be empty".to_owned());
                }
                request.sender = raw.clone();
            }
            "--preset" => {
                let raw = rest.next().ok_or("--preset: expected a preset name")?;
                if raw.trim().is_empty() {
                    return Err("--preset: the name cannot be empty".to_owned());
                }
                request.preset = Some(raw.clone());
            }
            "--frames" => {
                let raw = rest.next().ok_or("--frames: expected a count")?;
                let count: u64 = raw
                    .parse()
                    .map_err(|_| format!("--frames: '{raw}' is not a count"))?;
                if count == 0 {
                    return Err("--frames: a run of zero frames publishes nothing".to_owned());
                }
                request.frames = Some(count);
            }
            _ => {}
        }
    }
    Ok(Some(request))
}

fn parse_size(raw: &str) -> Result<(u32, u32), String> {
    let (w, h) = raw
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("--size: '{raw}' is not WIDTHxHEIGHT"))?;
    let width: u32 = w
        .trim()
        .parse()
        .map_err(|_| format!("--size: '{w}' is not a width"))?;
    let height: u32 = h
        .trim()
        .parse()
        .map_err(|_| format!("--size: '{h}' is not a height"))?;
    if width == 0 || height == 0 {
        return Err(format!("--size: {width}x{height} has no pixels"));
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format!(
            "--size: {width}x{height} exceeds the {MAX_DIMENSION} px limit"
        ));
    }
    Ok((width, height))
}

fn parse_fps(raw: &str) -> Result<u32, String> {
    let fps: u32 = raw
        .trim()
        .parse()
        .map_err(|_| format!("--fps: '{raw}' is not a frame rate"))?;
    if fps == 0 {
        return Err("--fps: a frame rate of zero emits nothing".to_owned());
    }
    if fps > MAX_FPS {
        return Err(format!("--fps: {fps} exceeds the {MAX_FPS} limit"));
    }
    Ok(fps)
}

/// How long to wait before emitting frame `index`, given the run's elapsed time.
///
/// `None` means the deadline has already passed and the next frame is due now.
/// Deadlines are absolute against the start of the run rather than cumulative
/// per-frame sleeps, so a slow frame does not push every later frame back.
pub fn rest_before(index: u64, period: Duration, elapsed: Duration) -> Option<Duration> {
    let due = Duration::from_nanos(period.as_nanos().saturating_mul(u128::from(index)) as u64);
    due.checked_sub(elapsed).filter(|rest| !rest.is_zero())
}

/// How often the run reports its resident set and per-stage costs, in frames.
/// 1800 is 30 s at 60 fps: often enough to watch a trend over a set, rare
/// enough that the reporting is not itself a cost.
pub const REPORT_EVERY: u64 = 1800;

/// Wall-clock spent in each stage of the frame path, accumulated since the last
/// report.
///
/// **Two stages, because the engine/sink boundary is the only one a caller-side
/// clock can see.** `render_tapped` encodes the draw, submits it and then blocks
/// mapping the readback, so there is no CPU-visible instant between "drew" and
/// "read back": the block absorbs the GPU execution and the transfer together,
/// and a timer around the encode alone would measure the encode. Splitting those
/// two needs GPU timestamp queries — a device feature and a `core` seam, not a
/// clock out here. The split that *is* available is the one that says whether
/// the sink limits the rate, which is the question the readback-versus-zero-copy
/// decision turns on (ADR-0125).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StageCosts {
    /// Time inside `render_tapped`: draw, submit and the blocking readback.
    pub render: Duration,
    /// Time inside the Spout send: the upload into the sender's own device.
    pub send: Duration,
    /// Frames these totals cover.
    pub frames: u64,
}

impl StageCosts {
    /// Mean per-frame cost of each stage over the accumulated window.
    pub fn line(&self) -> String {
        if self.frames == 0 {
            return "stream: no frames to cost".to_owned();
        }
        let per = |total: Duration| total.as_secs_f64() * 1000.0 / self.frames as f64;
        format!(
            "stream: render+readback {:.2} ms, spout send {:.2} ms, mean over {} frames",
            per(self.render),
            per(self.send),
            self.frames
        )
    }

    /// Start a fresh window, so each report covers the interval since the last
    /// one rather than the whole run to date.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Whether frame `frames` closes a reporting interval.
pub fn should_report(frames: u64, every: u64) -> bool {
    every != 0 && frames != 0 && frames.is_multiple_of(every)
}

/// The exit line: frames emitted, wall-clock elapsed, scene-clock elapsed.
///
/// The three exist so the frame-rate reading taken against this mode is a
/// measurement rather than an argument. Wall and scene diverging says the run
/// did not keep up; no threshold is asserted on either here.
pub fn summary(frames: u64, wall: Duration, scene: f64, adapter: &str) -> String {
    format!(
        "stream: {frames} frames, {:.2} s wall, {scene:.2} s scene clock, on {adapter}",
        wall.as_secs_f64()
    )
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Set by the console control handler so the loop can leave through its own
/// exit path and print the summary, rather than being torn down mid-frame.
#[cfg(all(feature = "spout", windows))]
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ctrl-C, Ctrl-Break and console close all mean "stop after this frame".
///
/// Returning `TRUE` claims the event, so the default terminate handler does not
/// run and the exit summary gets a chance to print. The handler runs on its own
/// thread and touches nothing but the flag.
#[cfg(all(feature = "spout", windows))]
unsafe extern "system" fn console_handler(_kind: u32) -> windows::core::BOOL {
    STOPPING.store(true, std::sync::atomic::Ordering::Relaxed);
    true.into()
}

/// Run the headless source until Ctrl-C or `--frames`.
#[cfg(all(feature = "spout", windows))]
pub fn run(
    request: &StreamRequest,
    input: &crate::config::Input,
    rotate: &crate::config::Rotate,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    use lmv_core::dsp::Analyzer;
    use lmv_core::render::{HeadlessOptions, Renderer, Tier};
    use standalone::gpu::{self, SenderAdapter};
    use standalone::shot::render::ResidentSet;
    use standalone::spout::{SpoutSender, adapters};

    // The renderer's adapter is a frame-rate choice; the sender's, below, is a
    // correctness one. Both come from one `--gpu`, resolved against their own
    // rosters (ADR-0146).
    let mut renderer = Renderer::new_headless_on(
        HeadlessOptions {
            width: request.width,
            height: request.height,
            prefer_software: false,
        },
        // Pinned rich: there is no frame-time governor on this path, so the
        // tier cannot demote itself mid-run the way the window's auto tier can.
        Tier::Rich,
        &gpu::renderer_choice(request.gpu.as_deref()),
    )
    .map_err(|err| format!("--stream: {err}"))?;
    let adapter = renderer.adapter_description().to_owned();
    eprintln!("renderer : {adapter}");

    let roster = adapters();
    let sender_index = match gpu::sender_adapter(request.gpu.as_deref(), &adapter, &roster) {
        Ok(SenderAdapter::Pinned(index)) => {
            eprintln!(
                "sender   : adapter [{index}] {}",
                roster.get(index as usize).map_or("?", String::as_str)
            );
            Some(index)
        }
        // Never silent: on a hybrid machine the D3D11 default is the
        // power-saving GPU, and a receiver on the other one reports only that
        // it could not open the texture.
        Ok(SenderAdapter::Default { reason }) => {
            eprintln!("sender   : the D3D11 default - {reason}");
            None
        }
        Err(message) => return Err(format!("--stream: {message}")),
    };
    if request.gpu.is_none() && roster.len() > 1 {
        eprintln!(
            "note     : this machine has {} graphics adapters and --gpu was not given. If the \
             receiver cannot open the sender, re-run with --gpu naming the GPU it renders on.",
            roster.len()
        );
    }

    let capture = crate::start_capture(input);
    let Some(mut consumer) = capture.consumer else {
        return Err(
            "--stream: no audio capture device is available, so there is nothing to visualize"
                .to_owned(),
        );
    };
    let mut analyzer = Analyzer::new(capture.format)
        .map_err(|err| format!("--stream: the capture format is unusable: {err}"))?;
    // Held for the run: dropping the handle stops the stream.
    let _capture_handle = capture.handle;

    let mut tap = renderer.open_tap();
    let mut sender = SpoutSender::new(&request.sender, request.width, request.height, sender_index)
        .map_err(|err| format!("--stream: {err}"))?;
    eprintln!(
        "publishing {}x{} at {} fps as Spout sender '{}'",
        request.width,
        request.height,
        request.fps,
        sender.name()
    );

    // SAFETY: registering a console handler with no state of its own; the
    // callback writes one atomic and returns.
    if unsafe {
        windows::Win32::System::Console::SetConsoleCtrlHandler(Some(console_handler), true)
    }
    .is_err()
    {
        eprintln!("note     : could not install a Ctrl-C handler; the exit summary may not print");
    }

    // A headless source has nobody to press Space, so rotation is ON here even
    // though `[rotate] auto` defaults off for the window (ADR-0027): a source
    // stuck on one preset for a four-hour set is not what this mode is for. The
    // dwell bounds still come from the operator's config, and `--preset` opts
    // out entirely.
    let mut director = crate::director::Director::from_config(&crate::config::Rotate {
        auto: request.preset.is_none(),
        ..rotate.clone()
    });
    if let Some(name) = request.preset.as_deref() {
        if !renderer.select_preset_by_name(name) {
            return Err(format!(
                "--stream: no preset named '{name}'; --list-presets is not a flag, but the embedded set is what the window browses"
            ));
        }
        eprintln!("preset   : '{name}', held for the run - rotation is off");
    } else {
        eprintln!(
            "rotation : on, dwell {}-{} s from the operator config",
            rotate.min_dwell_secs, rotate.max_dwell_secs
        );
    }

    let period = request.period();
    let mut scratch = vec![0.0_f32; 32_768];
    let mut frames: u64 = 0;
    let mut scene = 0.0_f64;
    let mut costs = StageCosts::default();
    let mut resident = ResidentSet::default();
    resident.sample();

    // Frame pacing is a shell concern; the core stays clock-free.
    #[allow(
        clippy::disallowed_methods,
        reason = "stream pacing reads the wall clock; core analysis stays clock-free"
    )]
    let started = Instant::now();
    let mut last = started;

    loop {
        if STOPPING.load(Ordering::Relaxed) {
            break;
        }
        if request.frames.is_some_and(|limit| frames >= limit) {
            break;
        }

        // Drain everything the capture callback has handed over since the last
        // frame. The callback never blocks; this side does all the work.
        loop {
            let n = consumer.pop_samples(&mut scratch);
            if n == 0 {
                break;
            }
            analyzer.push_interleaved(scratch.get(..n).unwrap_or_default());
        }
        let frame = analyzer.take_frame();

        #[allow(
            clippy::disallowed_methods,
            reason = "stream pacing reads the wall clock; core analysis stays clock-free"
        )]
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32();
        last = now;
        scene += f64::from(dt);

        // Hands-off rotation, on the same director the window runs. The
        // decision and the change are paired here for the same reason the
        // shell pairs them: a rotation that is announced and not carried out
        // leaves the source on one scene for the whole set.
        if let Some(reason) = director.advance(dt, &frame) {
            let incoming = renderer.cycle_preset().to_owned();
            eprintln!("rotate   : frame {frames}, {reason:?} -> '{incoming}'");
        }

        #[allow(
            clippy::disallowed_methods,
            reason = "stage costing reads the wall clock; core analysis stays clock-free"
        )]
        let drew = Instant::now();
        let image = renderer
            .render_tapped(&mut tap, &frame, dt)
            .map_err(|err| format!("--stream: frame {frames}: {err}"))?;
        #[allow(
            clippy::disallowed_methods,
            reason = "stage costing reads the wall clock; core analysis stays clock-free"
        )]
        let sent = Instant::now();
        sender
            .send(&image.rgba, image.width, image.height)
            .map_err(|err| format!("--stream: frame {frames}: {err}"))?;
        #[allow(
            clippy::disallowed_methods,
            reason = "stage costing reads the wall clock; core analysis stays clock-free"
        )]
        let done = Instant::now();
        costs.render += sent.duration_since(drew);
        costs.send += done.duration_since(sent);
        costs.frames += 1;
        frames += 1;

        if should_report(frames, REPORT_EVERY) {
            resident.sample();
            eprintln!("{}", costs.line());
            eprintln!(
                "{}",
                resident.summary(frames.min(u64::from(u32::MAX)) as u32)
            );
            costs.reset();
        }

        #[allow(
            clippy::disallowed_methods,
            reason = "stream pacing reads the wall clock; core analysis stays clock-free"
        )]
        let elapsed = started.elapsed();
        if let Some(rest) = rest_before(frames, period, elapsed) {
            std::thread::sleep(rest);
        }
    }

    #[allow(
        clippy::disallowed_methods,
        reason = "stream pacing reads the wall clock; core analysis stays clock-free"
    )]
    let wall = started.elapsed();
    resident.sample();
    if costs.frames > 0 {
        eprintln!("{}", costs.line());
    }
    eprintln!(
        "{}",
        resident.summary(frames.min(u64::from(u32::MAX)) as u32)
    );
    eprintln!("{}", summary(frames, wall, scene, &adapter));
    Ok(())
}

/// Without the `spout` feature (or off Windows) there is no sink, and the mode
/// says so rather than starting and publishing nowhere.
#[cfg(not(all(feature = "spout", windows)))]
pub fn run(
    _request: &StreamRequest,
    _input: &crate::config::Input,
    _rotate: &crate::config::Rotate,
) -> Result<(), String> {
    Err(
        "--stream needs a build with the 'spout' feature on Windows; this binary was built \
         without it, so there is no Spout sender to publish to"
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn without_the_flag_there_is_no_request() {
        assert_eq!(parse(&args(&["--fps", "60"])), Ok(None));
    }

    #[test]
    fn the_defaults_are_the_documented_ones() {
        let request = parse(&args(&["--stream"]))
            .expect("a bare --stream parses")
            .expect("--stream was given");
        assert_eq!(request.width, 1280);
        assert_eq!(request.height, 720);
        assert_eq!(request.fps, 60);
        assert_eq!(request.sender, "lmv");
        assert_eq!(request.gpu, None);
        assert_eq!(request.frames, None);
    }

    #[test]
    fn the_documented_invocation_parses() {
        let request = parse(&args(&[
            "--stream", "--size", "1280x720", "--fps", "60", "--gpu", "RTX 3080",
        ]))
        .expect("valid")
        .expect("--stream was given");
        assert_eq!(request.width, 1280);
        assert_eq!(request.height, 720);
        assert_eq!(request.fps, 60);
        assert_eq!(request.gpu.as_deref(), Some("RTX 3080"));
    }

    #[test]
    fn a_size_needs_both_dimensions_and_they_must_have_pixels() {
        assert!(parse(&args(&["--stream", "--size", "1280"])).is_err());
        assert!(parse(&args(&["--stream", "--size", "0x720"])).is_err());
        assert!(parse(&args(&["--stream", "--size", "1280x0"])).is_err());
        assert!(parse(&args(&["--stream", "--size", "wide"])).is_err());
    }

    #[test]
    fn an_absurd_size_or_rate_is_refused_rather_than_attempted() {
        let err = parse(&args(&["--stream", "--size", "99999x720"]))
            .expect_err("99999 px exceeds the limit");
        assert!(err.contains("exceeds"), "{err}");
        let err = parse(&args(&["--stream", "--fps", "6000"])).expect_err("6000 fps is a typo");
        assert!(err.contains("exceeds"), "{err}");
    }

    #[test]
    fn a_flag_missing_its_value_is_an_error_and_not_a_default() {
        assert!(parse(&args(&["--stream", "--size"])).is_err());
        assert!(parse(&args(&["--stream", "--fps"])).is_err());
        assert!(parse(&args(&["--stream", "--gpu"])).is_err());
        assert!(parse(&args(&["--stream", "--frames"])).is_err());
    }

    #[test]
    fn a_zero_frame_run_is_refused() {
        assert!(parse(&args(&["--stream", "--frames", "0"])).is_err());
        assert_eq!(
            parse(&args(&["--stream", "--frames", "600"]))
                .expect("valid")
                .expect("--stream was given")
                .frames,
            Some(600)
        );
    }

    #[test]
    fn the_period_is_the_reciprocal_of_the_rate() {
        let request = StreamRequest {
            fps: 60,
            ..StreamRequest::default()
        };
        assert_eq!(request.period(), Duration::from_nanos(16_666_666));
    }

    /// The property the deadline arithmetic exists for: frame `n` is due at
    /// `n * period` from the start, so one slow frame costs only itself.
    #[test]
    fn a_slow_frame_does_not_push_every_later_frame_back() {
        let period = Duration::from_nanos(16_666_666);
        // Frame 10 is due at ~166.67 ms whatever happened before it.
        let due_at_10 = Duration::from_nanos(166_666_660);
        // A run that is 100 ms in still waits the remainder.
        let rest = rest_before(10, period, Duration::from_millis(100))
            .expect("frame 10 is not due at 100 ms");
        assert_eq!(rest, due_at_10 - Duration::from_millis(100));
        // A run that already overran past the deadline does not sleep at all.
        assert_eq!(rest_before(10, period, Duration::from_millis(200)), None);
    }

    #[test]
    fn the_first_frame_is_due_immediately() {
        let period = Duration::from_nanos(16_666_666);
        assert_eq!(rest_before(0, period, Duration::ZERO), None);
    }

    /// A four-hour run at 60 fps is 864,000 frames; the deadline for the last
    /// one must still be computed exactly rather than overflow.
    #[test]
    fn a_long_run_computes_its_deadlines_without_overflow() {
        let period = Duration::from_nanos(16_666_666);
        let last = 864_000_u64;
        let rest =
            rest_before(last, period, Duration::ZERO).expect("the deadline is in the future");
        assert_eq!(rest, Duration::from_nanos(16_666_666 * last));
        assert!(rest.as_secs() > 14_000, "four hours of frames: {rest:?}");
    }

    #[test]
    fn a_pinned_preset_parses_and_is_refused_empty() {
        let request = parse(&args(&["--stream", "--preset", "attractor_ink"]))
            .expect("valid")
            .expect("--stream was given");
        assert_eq!(request.preset.as_deref(), Some("attractor_ink"));
        assert!(parse(&args(&["--stream", "--preset", "  "])).is_err());
        assert!(parse(&args(&["--stream", "--preset"])).is_err());
    }

    #[test]
    fn with_no_pinned_preset_there_is_none_to_hold() {
        assert_eq!(
            parse(&args(&["--stream"]))
                .expect("valid")
                .expect("--stream was given")
                .preset,
            None
        );
    }

    #[test]
    fn a_report_closes_each_interval_and_nothing_between() {
        assert!(!should_report(0, 1800), "frame zero closes no interval");
        assert!(!should_report(1799, 1800));
        assert!(should_report(1800, 1800));
        assert!(!should_report(1801, 1800));
        assert!(should_report(3600, 1800));
    }

    /// A zero interval must not divide by zero or report every frame.
    #[test]
    fn a_zero_interval_reports_never() {
        assert!(!should_report(1800, 0));
    }

    #[test]
    fn the_stage_line_reports_a_mean_per_frame_for_each_stage() {
        let costs = StageCosts {
            // 100 frames costing 8 ms each in the engine, 1 ms each in the sink.
            render: Duration::from_millis(800),
            send: Duration::from_millis(100),
            frames: 100,
        };
        let line = costs.line();
        assert!(line.contains("render+readback 8.00 ms"), "{line}");
        assert!(line.contains("spout send 1.00 ms"), "{line}");
        assert!(line.contains("over 100 frames"), "{line}");
    }

    #[test]
    fn an_empty_window_costs_nothing_rather_than_dividing_by_zero() {
        assert!(StageCosts::default().line().contains("no frames"));
    }

    /// Each report covers the interval since the last one, so a slow stretch
    /// shows up instead of being averaged away by everything before it.
    #[test]
    fn resetting_starts_a_fresh_window() {
        let mut costs = StageCosts {
            render: Duration::from_millis(800),
            send: Duration::from_millis(100),
            frames: 100,
        };
        costs.reset();
        assert_eq!(costs, StageCosts::default());
        assert_eq!(costs.frames, 0);
    }

    #[test]
    fn the_summary_carries_all_three_numbers_and_the_adapter() {
        let line = summary(
            36_000,
            Duration::from_millis(600_120),
            600.05,
            "NVIDIA RTX 3080",
        );
        assert!(line.contains("36000 frames"), "{line}");
        assert!(line.contains("600.12 s wall"), "{line}");
        assert!(line.contains("600.05 s scene clock"), "{line}");
        assert!(line.contains("NVIDIA RTX 3080"), "{line}");
    }
}
