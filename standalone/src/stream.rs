//! `ritmolux --stream`: the headless live video source (ADR-0125).
//!
//! No window and no swapchain. Loopback audio drives the analyzer, the renderer
//! draws through the same `draw_frame` the window presents through, the frame
//! tap reads each frame back, and a **sink** publishes it.
//!
//! ## Two sinks, one loop
//!
//! [`Sink::Spout`] hands the frame to a Spout sender, which TouchDesigner picks
//! up with a `Syphon Spout In` TOP on the same machine. It needs Windows and the
//! `spout` feature. [`Sink::Stdout`] writes the raw pixels to standard output,
//! which needs neither and works wherever the player runs — it is how a parent
//! process that spawned this player watches what it is drawing.
//!
//! Everything between the analyzer and the sink is identical for both, which is
//! the point of the trait: the cost line below reports the same two stages
//! whichever is open, so the readback-versus-zero-copy question ADR-0125 leaves
//! open has one measurement rather than two.
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

use std::io::Write;
use std::time::Duration;

use rlx_core::render::PixelOrder;
use standalone::events::Events;

/// The sender name a receiver lists, unless `--sender` overrides it. Not
/// necessarily the name that gets registered: a stale registration from a
/// crashed run makes `SetSenderName` increment, which is why the mode prints
/// what it actually got.
pub const DEFAULT_SENDER: &str = "Ritmolux";

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;
const DEFAULT_FPS: u32 = 60;

/// The pipe sink's own defaults: a preview, not a second show.
///
/// 640x360 at 30 fps is what a studio's canvas actually displays, and asking the
/// engine for a 1280x720 frame sixty times a second to shrink it into a panel
/// would cost the readback and the pipe four times over for a picture nobody
/// sees at that size. `--size` and `--fps` still say otherwise where a caller
/// wants them to.
const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 360;
const PREVIEW_FPS: u32 = 30;

/// The same default for the **windowed** mirror `--preview stdout` opens, which
/// is the other producer of a preview-shaped pipe. One constant rather than two
/// so the two paths cannot drift into different pictures of the same thing.
pub(crate) const DEFAULT_PREVIEW_SIZE: (u32, u32) = (PREVIEW_WIDTH, PREVIEW_HEIGHT);

/// Capacity reserved for the transport verbs one drained control frame carries,
/// matching the listener's own per-frame cap. A frame that somehow carried more
/// would grow the vector once and keep the capacity; the number is here so the
/// steady state never does.
const TRANSPORT_SCRATCH: usize = 8;

/// Where `--stream` publishes its frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sink {
    /// A Spout sender another application on the same machine opens by name.
    /// Windows, and a build with the `spout` feature.
    #[default]
    Spout,
    /// Raw frames on standard output, in order, with nothing between them.
    ///
    /// Every platform and no feature: a parent that spawned this player reads
    /// its own child's pipe, which needs no shared-texture API and no name to
    /// collide on.
    Stdout,
}

impl Sink {
    /// Parse a `--sink` value, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "spout" => Some(Sink::Spout),
            "stdout" => Some(Sink::Stdout),
            _ => None,
        }
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse, and what
    /// the usage error lists.
    pub fn as_str(self) -> &'static str {
        match self {
            Sink::Spout => "spout",
            Sink::Stdout => "stdout",
        }
    }

    /// Every sink, for the usage error and the round-trip test.
    pub const ALL: [Sink; 2] = [Sink::Spout, Sink::Stdout];

    /// The size and rate a request takes when the caller named neither.
    fn default_geometry(self) -> (u32, u32, u32) {
        match self {
            Sink::Spout => (DEFAULT_WIDTH, DEFAULT_HEIGHT, DEFAULT_FPS),
            Sink::Stdout => (PREVIEW_WIDTH, PREVIEW_HEIGHT, PREVIEW_FPS),
        }
    }

    /// What the cost line calls this sink's send stage.
    fn send_label(self) -> &'static str {
        match self {
            Sink::Spout => "spout send",
            Sink::Stdout => "pipe write",
        }
    }
}

/// Upper bound on a requested frame rate. Not a capability claim - it rejects a
/// typo (`--fps 6000`) before it becomes a busy loop.
const MAX_FPS: u32 = 240;

/// Upper bound on a requested dimension, matching the largest target this
/// engine is exercised at with headroom. Rejects a transposed or mistyped size
/// before a multi-gigabyte allocation is attempted.
pub(crate) const MAX_DIMENSION: u32 = 7680;

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
    /// Where the frames go.
    pub sink: Sink,
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
            sink: Sink::Spout,
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
    // Held aside rather than written straight into the request: the size and
    // rate a caller did NOT name depend on the sink, and `--sink` may arrive
    // after them on the command line.
    let mut size = None;
    let mut fps = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--stream" => {}
            "--size" => {
                let raw = rest.next().ok_or("--size: expected WIDTHxHEIGHT")?;
                size = Some(parse_size(raw)?);
            }
            "--fps" => {
                let raw = rest.next().ok_or("--fps: expected a frame rate")?;
                fps = Some(parse_fps(raw)?);
            }
            "--sink" => {
                let raw = rest.next().ok_or("--sink: expected a sink name")?;
                request.sink = Sink::from_name(raw).ok_or_else(|| {
                    format!(
                        "--sink: unknown sink '{raw}' (expected one of: {})",
                        Sink::ALL
                            .iter()
                            .map(|sink| sink.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
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
    let (default_width, default_height, default_fps) = request.sink.default_geometry();
    let (width, height) = size.unwrap_or((default_width, default_height));
    request.width = width;
    request.height = height;
    request.fps = fps.unwrap_or(default_fps);
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
    /// Time inside the sink's send: the upload into a Spout sender's own
    /// device, or the blocking write into the pipe.
    pub send: Duration,
    /// Frames these totals cover.
    pub frames: u64,
}

impl StageCosts {
    /// Mean per-frame cost of each stage over the accumulated window.
    ///
    /// `sink` names the second stage, so a figure copied out of a log says which
    /// sink produced it — the two are not comparable, and a line that called a
    /// pipe write a Spout send would be read as though they were.
    pub fn line(&self, sink: Sink) -> String {
        if self.frames == 0 {
            return "stream: no frames to cost".to_owned();
        }
        let per = |total: Duration| total.as_secs_f64() * 1000.0 / self.frames as f64;
        format!(
            "stream: render+readback {:.2} ms, {} {:.2} ms, mean over {} frames",
            per(self.render),
            sink.send_label(),
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

/// Where a rendered frame goes.
///
/// A trait rather than an enum with two arms in the loop, because the two
/// implementations have nothing in common but this call: one needs a Windows
/// shared-texture API and a build feature, the other needs a file descriptor.
/// The loop names neither.
pub trait FrameSink {
    /// Publish one frame. `rgba` is tight `width * height * 4` bytes.
    fn send(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), String>;

    /// The startup line saying what was actually opened.
    fn opened(&self) -> String;
}

/// Raw frames on standard output.
///
/// **Blocking, never dropping.** A parent that stops reading stalls this writer,
/// and the deadline pacing above absorbs that: frame `n` stays due at
/// `n * period` from the start of the run, so a stalled second costs the frames
/// that fell inside it and the run resumes at the frame index the wall clock has
/// reached rather than drifting behind it. That is the right policy *here* and
/// the wrong one on a windowed run, which has a present deadline it cannot miss
/// (ADR-0125); the windowed preview drops instead, and the two are deliberate.
pub struct StdoutSink {
    /// The geometry announced before the first frame, so a frame of another size
    /// is refused rather than silently reinterpreted by whatever is reading.
    width: u32,
    height: u32,
    /// The channel order announced with it, carried so the opening line names
    /// what the bytes are rather than a format this sink assumed.
    order: PixelOrder,
}

impl StdoutSink {
    /// A sink that will write `width` x `height` frames of `order`.
    pub fn new(width: u32, height: u32, order: PixelOrder) -> Self {
        Self {
            width,
            height,
            order,
        }
    }
}

impl FrameSink for StdoutSink {
    fn send(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), String> {
        if (width, height) != (self.width, self.height) {
            return Err(format!(
                "the stream announced {}x{} and this frame is {width}x{height}; a reader \
                 taking fixed-size frames would resynchronize on nothing",
                self.width, self.height
            ));
        }
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(format!(
                "expected {expected} bytes for a {width}x{height} frame and the tap \
                 produced {}",
                rgba.len()
            ));
        }
        let mut out = std::io::stdout().lock();
        out.write_all(rgba)
            .map_err(|err| format!("writing a frame to standard output: {err}"))?;
        // Flushed per frame: a reader takes whole frames, and a partial one left
        // in a buffer is a reader blocked on bytes this process is holding.
        out.flush()
            .map_err(|err| format!("flushing a frame to standard output: {err}"))
    }

    fn opened(&self) -> String {
        format!(
            "publishing {}x{} as raw {} frames on standard output",
            self.width,
            self.height,
            self.order.as_str()
        )
    }
}

/// The Spout sender as a [`FrameSink`].
#[cfg(all(feature = "spout", windows))]
struct SpoutFrameSink {
    sender: standalone::spout::SpoutSender,
    width: u32,
    height: u32,
    fps: u32,
}

#[cfg(all(feature = "spout", windows))]
impl FrameSink for SpoutFrameSink {
    fn send(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), String> {
        self.sender
            .send(rgba, width, height)
            .map_err(|err| err.to_string())
    }

    fn opened(&self) -> String {
        format!(
            "publishing {}x{} at {} fps as Spout sender '{}'",
            self.width,
            self.height,
            self.fps,
            self.sender.name()
        )
    }
}

/// The windowed run's preview writer: a thread, a bounded queue, and a count of
/// what it could not take.
///
/// **This one drops; the headless sink blocks.** The two policies are opposite
/// and both are deliberate. A headless loop has no present deadline, so a
/// blocked write costs frames and nothing else. A windowed run has one it cannot
/// miss, and a reader that stopped reading must not be able to take the show
/// down with it — so the frame is dropped, counted, and the show carries on.
///
/// The count is what makes the cost measurable rather than assumed: a preview
/// that delivered nothing and a preview that cost nothing look identical without
/// it (ADR-0172).
pub struct PreviewPipe {
    tx: Option<std::sync::mpsc::SyncSender<rlx_core::render::CaptureImage>>,
    writer: Option<std::thread::JoinHandle<()>>,
    /// Frames the queue had no room for.
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Frames handed to the writer.
    sent: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

/// How many frames may wait for the writer.
///
/// Two: one being written and one behind it. A deeper queue would not help a
/// reader that is slower than the show — it would only make the picture it
/// eventually sees older.
const PREVIEW_QUEUE: usize = 2;

impl PreviewPipe {
    /// Start the writer thread for `width` x `height` frames.
    pub fn spawn(width: u32, height: u32, order: PixelOrder) -> Self {
        let (tx, rx) =
            std::sync::mpsc::sync_channel::<rlx_core::render::CaptureImage>(PREVIEW_QUEUE);
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let sent = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let writer = std::thread::Builder::new()
            .name("rlx-preview".to_owned())
            .spawn(move || {
                let mut sink = StdoutSink::new(width, height, order);
                // Ends when the sender is dropped, which is what closes this
                // down: no flag, no timeout, no way to leave the thread running
                // past the show.
                while let Ok(image) = rx.recv() {
                    if sink.send(&image.rgba, image.width, image.height).is_err() {
                        // The reader went away. Nothing to report to and nothing
                        // to do but stop taking frames; the show keeps drawing.
                        break;
                    }
                }
            })
            .ok();
        Self {
            tx: Some(tx),
            writer,
            dropped,
            sent,
        }
    }

    /// Hand a frame to the writer, or drop it and count that.
    pub fn send(&self, image: rlx_core::render::CaptureImage) {
        use std::sync::atomic::Ordering;
        let Some(tx) = self.tx.as_ref() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        match tx.try_send(image) {
            Ok(()) => {
                self.sent.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Frames written, and frames dropped — the pair a cost line reports.
    pub fn totals(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering;
        (
            self.sent.load(Ordering::Relaxed),
            self.dropped.load(Ordering::Relaxed),
        )
    }
}

impl Drop for PreviewPipe {
    /// Close the queue and wait for the writer to finish the frame it holds.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

/// Set by the console control handler so the loop can leave through its own
/// exit path and print the summary, rather than being torn down mid-frame.
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ctrl-C, Ctrl-Break and console close all mean "stop after this frame".
///
/// Returning `TRUE` claims the event, so the default terminate handler does not
/// run and the exit summary gets a chance to print. The handler runs on its own
/// thread and touches nothing but the flag.
#[cfg(windows)]
unsafe extern "system" fn console_handler(_kind: u32) -> windows::core::BOOL {
    STOPPING.store(true, std::sync::atomic::Ordering::Relaxed);
    true.into()
}

/// Install the stop-after-this-frame handler, or say why it could not be.
///
/// Windows-only because the flag it sets is: elsewhere the loop leaves on
/// `--frames` or on the default signal disposition, and a run with no frame
/// limit is torn down rather than summarised. Stated here rather than left as an
/// absent `cfg` so a reader on either platform can see which they have.
fn install_stop_handler() {
    #[cfg(windows)]
    {
        // SAFETY: registering a console handler with no state of its own; the
        // callback writes one atomic and returns.
        if unsafe {
            windows::Win32::System::Console::SetConsoleCtrlHandler(Some(console_handler), true)
        }
        .is_err()
        {
            eprintln!(
                "note     : could not install a Ctrl-C handler; the exit summary may not print"
            );
        }
    }
}

/// Open the sink `request` asks for.
///
/// `adapter` is the renderer's own adapter description, which only the Spout arm
/// reads: on a hybrid machine the sender's device decides whether a receiver can
/// open the texture at all (ADR-0146), and the pipe has no such question.
#[cfg_attr(
    not(all(feature = "spout", windows)),
    allow(
        unused_variables,
        reason = "only the Spout arm reads the renderer's adapter, and it is not compiled here"
    )
)]
fn open_sink(
    request: &StreamRequest,
    adapter: &str,
    order: PixelOrder,
) -> Result<Box<dyn FrameSink>, String> {
    match request.sink {
        Sink::Stdout => Ok(Box::new(StdoutSink::new(
            request.width,
            request.height,
            order,
        ))),
        #[cfg(all(feature = "spout", windows))]
        Sink::Spout => {
            use standalone::gpu::{self, SenderAdapter};
            use standalone::spout::{SpoutSender, adapters};

            let roster = adapters();
            let sender_index = match gpu::sender_adapter(request.gpu.as_deref(), adapter, &roster) {
                Ok(SenderAdapter::Pinned(index)) => {
                    eprintln!(
                        "sender   : adapter [{index}] {}",
                        roster.get(index as usize).map_or("?", String::as_str)
                    );
                    Some(index)
                }
                // Never silent: on a hybrid machine the D3D11 default is the
                // power-saving GPU, and a receiver on the other one reports only
                // that it could not open the sender.
                Ok(SenderAdapter::Default { reason }) => {
                    eprintln!("sender   : the D3D11 default - {reason}");
                    None
                }
                Err(message) => return Err(format!("--stream: {message}")),
            };
            if request.gpu.is_none() && roster.len() > 1 {
                eprintln!(
                    "note     : this machine has {} graphics adapters and --gpu was not given. \
                     If the receiver cannot open the sender, re-run with --gpu naming the GPU it \
                     renders on.",
                    roster.len()
                );
            }
            let sender =
                SpoutSender::new(&request.sender, request.width, request.height, sender_index)
                    .map_err(|err| format!("--stream: {err}"))?;
            Ok(Box::new(SpoutFrameSink {
                sender,
                width: request.width,
                height: request.height,
                fps: request.fps,
            }))
        }
        #[cfg(not(all(feature = "spout", windows)))]
        Sink::Spout => Err(
            "--stream --sink spout needs a build with the 'spout' feature on Windows; this \
             binary was built without it, so there is no Spout sender to publish to. \
             `--sink stdout` writes the frames to standard output on every platform."
                .to_owned(),
        ),
    }
}

/// Run the headless source until Ctrl-C or `--frames`.
pub fn run(
    request: &StreamRequest,
    config: &standalone::config::Config,
    events: Option<Events>,
    control: Option<standalone::control::Control>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    use rlx_core::dsp::Analyzer;
    use rlx_core::render::{HeadlessOptions, Renderer, Tier};
    use standalone::gpu;
    use standalone::shot::render::ResidentSet;

    // The renderer's adapter is a frame-rate choice; the sender's, resolved
    // inside `open_sink`, is a correctness one. Both come from one `--gpu`,
    // resolved against their own rosters (ADR-0146).
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

    // Read off the renderer, not named here: a headless run draws into the
    // offscreen format and is RGBA by construction, but the question and its one
    // source are the same on both run modes (ADR-0187).
    let order = renderer
        .pixel_order()
        .map_err(|err| format!("--stream: {err}"))?;
    let mut sink = open_sink(request, &adapter, order)?;

    let capture = crate::capture_start::start_capture(&config.input);
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
    eprintln!("{}", sink.opened());

    install_stop_handler();

    // The show: the preset directory and its watcher, rotation, the event
    // stream and the control listener, performed by the same code the window
    // runs (ADR-0181). A headless source has nobody to press Space, so rotation
    // is ON here even though `[rotate] auto` defaults off for the window
    // (ADR-0027): a source stuck on one preset for a four-hour set is not what
    // this mode is for. The dwell bounds still come from the operator's config,
    // and `--preset` opts out entirely.
    let mut show = crate::show::Show::start(
        &mut renderer,
        &standalone::config::Rotate {
            auto: request.preset.is_none(),
            ..config.rotate.clone()
        },
        events,
        control,
    );
    // Judged against the set the show just installed, which is the per-user
    // directory when one resolved and the embedded set otherwise.
    if let Some(name) = request.preset.as_deref() {
        if !renderer.select_preset_by_name(name) {
            return Err(format!(
                "--stream: no preset named '{name}'; --list-presets is not a flag, but the preset directory is what the window browses"
            ));
        }
        eprintln!("preset   : '{name}', held for the run - rotation is off");
    } else {
        eprintln!(
            "rotation : on, dwell {}-{} s from the operator config",
            config.rotate.min_dwell_secs, config.rotate.max_dwell_secs
        );
    }

    // **Before the first frame**, which is the contract: a parent reading the
    // pipe has to know the geometry before it has bytes to cut up, and a
    // `stream` that arrived after them would leave the first frame unreadable.
    //
    show.emit_stream(request.width, request.height, request.fps, order);
    // Sized once for the listener's per-frame transport cap, so a frame carrying
    // verbs reuses this rather than growing it.
    let mut transports = Vec::with_capacity(TRANSPORT_SCRATCH);

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

        // Control input, drained between frames and applied before anything this
        // frame reads a parameter (ADR-0176), in the fixed order spec 0003 sets:
        // the transport verbs first, because a preset switch drops every
        // override, then the rest.
        if show.take_control_transports(&mut transports) {
            for verb in &transports {
                apply_transport(*verb, &mut show, &mut renderer, config);
            }
            show.apply_control_rest(&mut renderer);
        }

        // The preset directory, polled on the same watcher the window runs, so
        // a file saved by an editor reaches this path within one poll interval
        // and its `roster` / `preset_error` reach whoever spawned it.
        show.poll_presets(&mut renderer);

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
        if let Some(reason) = show.director.advance(dt, &frame) {
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
        sink.send(&image.rgba, image.width, image.height)
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

        // The structured stream, after the frame that produced the figures it
        // reports (ADR-0176). Both are no-ops without `--events`.
        show.report_active_preset(&renderer);
        // No preview pipe on this path: the headless sink writes the frames
        // itself and blocks rather than dropping, so there is no producer-side
        // loss to report and `null` says so.
        show.report_health(&renderer, done, None);

        if should_report(frames, REPORT_EVERY) {
            resident.sample();
            eprintln!("{}", costs.line(request.sink));
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
        eprintln!("{}", costs.line(request.sink));
    }
    eprintln!(
        "{}",
        resident.summary(frames.min(u64::from(u32::MAX)) as u32)
    );
    eprintln!("{}", summary(frames, wall, scene, &adapter));
    Ok(())
}

/// Apply one `ctl/transport` verb to a run that has no window.
///
/// **The verb-to-action mapping is [`crate::console::action_for_transport`],
/// the same function a click on the operator console's transport strip goes
/// through** — spec 0003 requires one rule rather than two that agree today, and
/// the mapping is where the rule lives. What differs here is only the applier:
/// a headless run has no title to update, no soak log to note a switch in and no
/// window to redraw, so the three actions a transport verb can resolve to are
/// carried out directly on the renderer and the director.
///
/// `every_transport_action_is_applied` pins the `_` arm: an action a future verb
/// could resolve to and this applier ignores fails that test rather than going
/// silent here.
fn apply_transport(
    verb: standalone::osc::decode::Transport,
    show: &mut crate::show::Show,
    renderer: &mut rlx_core::render::Renderer,
    config: &standalone::config::Config,
) {
    use crate::console::ConsoleAction;
    use crate::settings::SettingsAction;

    let auto = show.director.auto_enabled();
    let view = headless_view(
        auto,
        renderer.tier(),
        config,
        &show.preset_dir().display().to_string(),
    );
    let Some(action) = crate::console::action_for_transport(verb, auto, &view) else {
        return;
    };
    match action {
        ConsoleAction::Next => {
            renderer.cycle_preset();
        }
        ConsoleAction::Prev => {
            let count = renderer.preset_names().count();
            if let Some(index) = crate::console::previous_index(count, renderer.active_index()) {
                renderer.select_preset(index);
            }
        }
        ConsoleAction::Settings(SettingsAction::ToggleAuto) => {
            show.director.toggle_auto();
        }
        _ => {}
    }
}

/// The settings view a run with no window honestly has.
///
/// Every field is what is actually true of this run rather than a placeholder:
/// there is no window, so it is not fullscreen and the display roster is empty;
/// there is no console; the capture path takes no operator selection here. It
/// exists because [`crate::console::action_for_transport`] resolves a verb
/// against the live values the settings menu displays, and that function is the
/// one mapping both run modes go through.
///
/// Takes the four live values rather than the run's objects, so the mapping can
/// be walked in a test without a GPU.
fn headless_view(
    auto_rotate: bool,
    tier: rlx_core::render::Tier,
    config: &standalone::config::Config,
    preset_dir: &str,
) -> crate::settings::SettingsView {
    crate::settings::SettingsView {
        tier,
        // Pinned by construction on this path: `run` asks for `Tier::Rich` and
        // there is no frame-time governor here to demote it.
        tier_state: crate::settings::TierState::Pinned,
        auto_rotate,
        min_dwell_secs: config.rotate.min_dwell_secs,
        max_dwell_secs: config.rotate.max_dwell_secs,
        fullscreen: false,
        display_index: 0,
        display_count: 0,
        display_name: String::new(),
        diagnostics: false,
        input_mode: config.input.mode,
        input_device_index: 0,
        input_device_count: 0,
        input_device_name: config.input.device.clone(),
        input_editable: false,
        preset_name: config.hud.preset_name,
        now_playing: config.hud.now_playing,
        console: false,
        preset_dir: preset_dir.to_owned(),
    }
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
        assert_eq!(request.sender, "Ritmolux");
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
        let line = costs.line(Sink::Spout);
        assert!(line.contains("render+readback 8.00 ms"), "{line}");
        assert!(line.contains("spout send 1.00 ms"), "{line}");
        assert!(line.contains("over 100 frames"), "{line}");
    }

    #[test]
    fn an_empty_window_costs_nothing_rather_than_dividing_by_zero() {
        assert!(
            StageCosts::default()
                .line(Sink::Spout)
                .contains("no frames")
        );
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

#[cfg(test)]
mod sink_tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_owned()).collect()
    }

    fn request(raw: &[&str]) -> StreamRequest {
        parse(&args(raw))
            .expect("valid arguments")
            .expect("--stream was given")
    }

    /// Every sink round-trips its name, so the usage error's roster and the
    /// parser cannot list different things.
    #[test]
    fn every_sink_round_trips_its_name() {
        for sink in Sink::ALL {
            assert_eq!(
                Sink::from_name(sink.as_str()),
                Some(sink),
                "`{}` does not parse back to itself",
                sink.as_str()
            );
        }
        assert_eq!(Sink::from_name("syphon"), None);
    }

    /// The default sink is Spout, so a command line that says nothing about it
    /// asks for exactly what it asked for before the flag existed.
    #[test]
    fn the_default_sink_leaves_the_spout_path_as_it_was() {
        let request = request(&["--stream"]);
        assert_eq!(request.sink, Sink::Spout);
        assert_eq!(
            (request.width, request.height, request.fps),
            (1280, 720, 60)
        );
    }

    /// The pipe sink is a **preview**: smaller and slower by default, because a
    /// studio's canvas displays it at that size and asking the engine for four
    /// times the pixels would cost the readback and the pipe for a picture
    /// nobody sees.
    #[test]
    fn the_pipe_sink_defaults_to_the_preview_geometry() {
        let request = request(&["--stream", "--sink", "stdout"]);
        assert_eq!(request.sink, Sink::Stdout);
        assert_eq!((request.width, request.height, request.fps), (640, 360, 30));
    }

    /// An explicit size or rate wins over the sink's default, **whichever order
    /// they appear in** — the flag that decides the default can arrive last.
    #[test]
    fn an_explicit_size_or_rate_wins_over_the_sinks_default() {
        for raw in [
            &[
                "--stream",
                "--sink",
                "stdout",
                "--size",
                "1920x1080",
                "--fps",
                "24",
            ][..],
            &[
                "--stream",
                "--size",
                "1920x1080",
                "--fps",
                "24",
                "--sink",
                "stdout",
            ][..],
        ] {
            let request = request(raw);
            assert_eq!(
                (request.width, request.height, request.fps),
                (1920, 1080, 24),
                "the sink's default overrode an explicit request in {raw:?}"
            );
        }
    }

    /// An unknown sink is a usage error naming the ones that exist, rather than
    /// a silent fall back to the default.
    #[test]
    fn an_unknown_sink_is_refused_and_the_roster_named() {
        let err = parse(&args(&["--stream", "--sink", "syphon"]))
            .expect_err("`syphon` is not a sink this build has");
        assert!(
            err.contains("syphon"),
            "the message does not name the value: {err}"
        );
        for sink in Sink::ALL {
            assert!(
                err.contains(sink.as_str()),
                "the message does not offer `{}`: {err}",
                sink.as_str()
            );
        }
    }

    /// The cost line names the sink it measured, so a figure copied out of a log
    /// says which one produced it.
    #[test]
    fn the_cost_line_names_the_sink_it_measured() {
        let costs = StageCosts {
            render: Duration::from_millis(20),
            send: Duration::from_millis(10),
            frames: 10,
        };
        let spout = costs.line(Sink::Spout);
        let pipe = costs.line(Sink::Stdout);
        assert!(spout.contains("spout send"), "{spout}");
        assert!(pipe.contains("pipe write"), "{pipe}");
        // Both report the stage that is the same either way, at the same figure.
        for line in [&spout, &pipe] {
            assert!(line.contains("render+readback 2.00 ms"), "{line}");
        }
    }

    /// A frame of the wrong size is refused rather than written, because a
    /// reader taking fixed-size frames would resynchronize on nothing.
    #[test]
    fn the_pipe_sink_refuses_a_frame_of_another_size() {
        let mut sink = StdoutSink::new(4, 2, PixelOrder::Rgba8);
        let right = vec![0u8; 4 * 2 * 4];
        assert!(
            sink.send(&right[..right.len() - 4], 4, 2).is_err(),
            "a short buffer for the announced size was accepted"
        );
        assert!(
            sink.send(&right, 8, 1).is_err(),
            "a frame of another shape was accepted"
        );
    }

    /// Every action a transport verb can resolve to is one `apply_transport`
    /// carries out.
    ///
    /// The applier's `_` arm is the risk this pins: a verb that started
    /// resolving to `Random` or `RotateNow` would be accepted, counted as
    /// applied and do nothing, which is the silent shape ADR-0181 exists to
    /// stop. The mapping is walked rather than read, so widening it here fails
    /// rather than diverging.
    #[test]
    fn every_transport_action_is_applied() {
        use crate::console::ConsoleAction;
        use crate::settings::SettingsAction;
        use standalone::osc::decode::Transport;

        let config = standalone::config::Config::default();
        for verb in [
            Transport::Next,
            Transport::Prev,
            Transport::Auto,
            Transport::Hold,
        ] {
            for auto in [true, false] {
                // The view the applier itself builds, so the test walks the
                // mapping through the same values the run gives it.
                let view = headless_view(auto, rlx_core::render::Tier::Rich, &config, "");
                let Some(action) = crate::console::action_for_transport(verb, auto, &view) else {
                    continue;
                };
                assert!(
                    matches!(
                        action,
                        ConsoleAction::Next
                            | ConsoleAction::Prev
                            | ConsoleAction::Settings(SettingsAction::ToggleAuto)
                    ),
                    "`{}` with auto={auto} resolves to {action:?}, which \
                     `apply_transport` ignores",
                    verb.as_str()
                );
            }
        }
    }
}
