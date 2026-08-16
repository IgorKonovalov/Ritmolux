//! The `shot --render` offline video mode: walk a WAV clip at a fixed frame
//! step and stream frames to an external encoder (Plan 0101 / ADR-0114).
//!
//! **What makes this worth having.** Every live visualizer's render loop is
//! welded to a real-time audio device, so "export" means screen-capturing the
//! window and keeping whatever the machine managed. Nothing here races a
//! display: `dt` is injected, the DSP is a pure function of its input window,
//! and the grammar's randomness is pinned — so a render cannot drop a frame, is
//! not capped by the refresh rate, and produces the same bytes twice.
//!
//! **No encoder ships.** A 1080p RGBA frame is 8.29 MB and four minutes at 60 fps
//! is 119 GB, so the frames can never touch disk before the encoder; and a static
//! encoder is larger than this application's whole size budget ([NFR §4]). Frames
//! go out over a **pipe**, in a self-describing stream a user's own `ffmpeg`
//! reads natively.
//!
//! ## Two clocks
//!
//! The analysis hop cadence and the frame cadence are **different clocks** and
//! conflating them is the mistake this module exists to avoid. [`HOP_SIZE`] at
//! the clip's sample rate sets one — 93.75 Hz for 48 kHz audio — and `--fps` sets
//! the other. [`hops_through`] is the map between them, in integer arithmetic so
//! it cannot drift over a four-minute clip, and the hops-per-clip division itself
//! is [`film::total_hops`], not a second copy of it.
//!
//! ## The wire format
//!
//! **Y4M** (`ffmpeg -f yuv4mpegpipe`), chosen over NUT because its header is
//! thirty lines to parse in any language and one non-`ffmpeg` consumer is already
//! foreseen ([Plan 0106](../../../docs/plans/0106-the-frame-stream-passes-through-a-diffusion-model.md)).
//! The cost is that Y4M cannot carry RGB — the muxer *errors* on `rgb24` — so
//! this module owns an RGB→YUV conversion, at `C444` (no chroma subsampling) and
//! full range, declared as `XCOLORRANGE=FULL`. That conversion is not bijective
//! at 8 bits, which is why the tap-placement assertion is made on the RGB frame
//! handed to [`write_y4m_frame`] and never on the wire bytes: a guard written
//! against the wire would have to be loosened to a tolerance until it passed.
//!
//! [NFR §4]: ../../../docs/nfr.md#4-size-and-dependencies

use std::io::Write;

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, Analyzer, HOP_SIZE};
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, Renderer, Tier};

use super::film::total_hops;

/// Frame rate as an exact rational, because the stream header carries one.
///
/// Not an `f32`: the two rates broadcast video actually uses — 30000/1001 and
/// 24000/1001 — are not representable, and a header claiming `29.97` when the
/// loop stepped by `1.0 / 29.97` is a drift of a frame every few minutes against
/// the soundtrack. The rational is the same number in both places by
/// construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fps {
    /// Frames per `den` seconds. Non-zero.
    pub num: u32,
    /// Seconds the numerator counts over. Non-zero.
    pub den: u32,
}

/// Sixty frames a second — the rate every other capture path in this repo runs
/// at, so a rendered frame is directly comparable with a `--frame-at` PNG.
pub const DEFAULT_FPS: Fps = Fps { num: 60, den: 1 };

impl Fps {
    /// The step to advance the renderer by, in seconds.
    ///
    /// At 60 fps this is bit-for-bit `scenes::FALLBACK_DT` (`1.0 / 60.0`), which
    /// is what lets Phase 3's byte-identity assertion hold at all.
    pub fn dt(self) -> f32 {
        self.den as f32 / self.num as f32
    }

    /// How it is spelled in the Y4M header's `F` field.
    pub fn as_header_field(self) -> String {
        format!("{}:{}", self.num, self.den)
    }
}

/// Parse `--fps` as a whole number of frames a second, or as an exact
/// `num/den` rational.
///
/// A decimal is **rejected rather than approximated**: `29.97` is not the rate
/// anybody means (it is 30000/1001), and silently rounding it to 2997/100 would
/// put a slow drift between the picture and its own soundtrack that nothing in
/// this harness would ever catch. The error says what to write instead.
pub fn parse_fps(spec: &str) -> Result<Fps, String> {
    let (num, den) = match spec.split_once('/') {
        Some((n, d)) => (n.trim(), d.trim()),
        None => (spec.trim(), "1"),
    };
    let parse = |s: &str, what: &str| -> Result<u32, String> {
        s.parse::<u32>().map_err(|_| {
            format!(
                "--fps expects a whole rate or an exact `num/den` rational \
                 (60, 30, 30000/1001), got `{spec}` — bad {what}"
            )
        })
    };
    let fps = Fps {
        num: parse(num, "numerator")?,
        den: parse(den, "denominator")?,
    };
    if fps.num == 0 || fps.den == 0 {
        return Err(format!("--fps `{spec}`: neither term may be zero"));
    }
    Ok(fps)
}

// ---------------------------------------------------------------------------
// The two clocks
// ---------------------------------------------------------------------------

/// Interleaved samples one analysis hop consumes at `format`.
fn hop_samples(format: AudioFormat) -> usize {
    HOP_SIZE * format.channels.max(1) as usize
}

/// Frames a clip of `pcm_len` interleaved samples renders to at `fps`.
///
/// `ceil(clip_seconds x fps)` — the last, partial frame is rendered rather than
/// dropped, so a render is never shorter than the audio it is muxed against.
/// Integer arithmetic throughout: a float `ceil` of a long clip is one rounding
/// away from an off-by-one, and the frame count is the thing the whole mode is
/// asserted on.
pub fn frame_count(pcm_len: usize, format: AudioFormat, fps: Fps) -> Result<u32, String> {
    let audio_frames = (pcm_len / format.channels.max(1) as usize) as u64;
    let per_second = u64::from(format.sample_rate) * u64::from(fps.den);
    if per_second == 0 {
        return Err("clip has a zero sample rate".to_string());
    }
    let frames = (audio_frames * u64::from(fps.num)).div_ceil(per_second);
    if frames == 0 {
        return Err(format!(
            "--render: the clip is shorter than one frame at {} fps",
            fps.as_header_field()
        ));
    }
    u32::try_from(frames).map_err(|_| format!("--render: {frames} frames is past what u32 holds"))
}

/// Analysis hops that have **completed** by the end of output frame `index`.
///
/// Frame `index` covers simulated time up to `(index + 1) * den / num` seconds,
/// and hop `h` completes at `(h + 1) * HOP_SIZE / sample_rate`. Both sides in
/// `u64` so a four-minute clip's 14,400th frame lands on exactly the hop it
/// should rather than one either side of it.
///
/// The result is **not** clamped to the clip: the caller knows how many hops the
/// PCM actually holds and the trailing partial frame legitimately asks for one
/// past the end.
pub fn hops_through(index: u32, fps: Fps, format: AudioFormat) -> usize {
    let elapsed_num = u64::from(index) + 1;
    let hops = elapsed_num * u64::from(fps.den) * u64::from(format.sample_rate)
        / (u64::from(fps.num) * HOP_SIZE as u64);
    usize::try_from(hops).unwrap_or(usize::MAX)
}

// ---------------------------------------------------------------------------
// The Y4M wire format
// ---------------------------------------------------------------------------

/// The stream header: dimensions, rate, interlacing, pixel aspect, chroma
/// layout, colour range.
///
/// Self-describing is the whole requirement (ADR-0114): a downstream consumer
/// reads the geometry off the stream, so a mistyped `-s`/`-pix_fmt` cannot
/// silently produce garbage. `Ip` is progressive, `A1:1` square pixels, `C444`
/// full chroma resolution, and `XCOLORRANGE=FULL` says the samples use 0-255
/// rather than the 16-235 studio swing — omit that and every exported frame is
/// visibly washed out against the app, which is exactly the failure ADR-0114
/// warns is most likely to ship unnoticed.
pub fn y4m_header(width: u32, height: u32, fps: Fps) -> String {
    format!(
        "YUV4MPEG2 W{width} H{height} F{} Ip A1:1 C444 XCOLORRANGE=FULL\n",
        fps.as_header_field()
    )
}

/// Full-range BT.709 luma and chroma for one 8-bit RGB pixel.
///
/// Full range rather than studio swing because the tap hands us display-referred
/// sRGB that already uses the whole 0-255 span; compressing it to 16-235 here
/// and expanding it in the player is two lossy steps to arrive back where we
/// started, with banding to show for it.
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    (
        round_u8(y),
        round_u8((b - y) / 1.8556 + 128.0),
        round_u8((r - y) / 1.5748 + 128.0),
    )
}

/// The inverse of [`rgb_to_yuv`], to the extent an 8-bit conversion has one.
///
/// It exists for the round-trip property (Plan 0101 Phase 3): the pair has to be
/// near-identity, and "near" is measurable. Nothing in the render path calls it —
/// the encoder owns the other direction.
pub fn yuv_to_rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let y = f32::from(y);
    let (u, v) = (f32::from(u) - 128.0, f32::from(v) - 128.0);
    (
        round_u8(y + 1.5748 * v),
        round_u8(y - 0.187_324 * u - 0.468_124 * v),
        round_u8(y + 1.8556 * u),
    )
}

/// Round to the nearest 8-bit level, clamping rather than wrapping — the chroma
/// terms of a saturated pixel legitimately land outside `0..=255`.
fn round_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Convert one captured RGBA frame into planar YUV 4:4:4 in `planes`, which is
/// resized and overwritten (reused across frames so a long render allocates
/// nothing per frame).
///
/// Alpha is dropped: the composite has already resolved onto the frame's own
/// ground by the time the tap sees it, so there is nothing for it to mean.
pub fn to_yuv444(img: &CaptureImage, planes: &mut Vec<u8>) {
    let pixels = img.width as usize * img.height as usize;
    planes.clear();
    planes.resize(pixels * 3, 0);
    let (luma, chroma) = planes.split_at_mut(pixels);
    let (cb, cr) = chroma.split_at_mut(pixels);
    for (i, px) in img.rgba.chunks_exact(4).take(pixels).enumerate() {
        let (y, u, v) = rgb_to_yuv(px[0], px[1], px[2]);
        luma[i] = y;
        cb[i] = u;
        cr[i] = v;
    }
}

/// Write one frame's `FRAME` marker and its three planes.
pub fn write_y4m_frame(
    img: &CaptureImage,
    planes: &mut Vec<u8>,
    out: &mut impl Write,
) -> std::io::Result<()> {
    to_yuv444(img, planes);
    out.write_all(b"FRAME\n")?;
    out.write_all(planes)
}

// ---------------------------------------------------------------------------
// The render loop
// ---------------------------------------------------------------------------

/// What the CLI asked for. The example owns the flags; this owns the shape, so
/// the library never depends on how a command line is spelled.
pub struct RenderRequest {
    /// Preset to render, or `None` when the roster names itself (one entry).
    pub preset: Option<String>,
    pub fps: Fps,
    pub width: u32,
    pub height: u32,
    pub tier: Tier,
}

/// Drive `name` over `pcm` at `fps`, handing every rendered frame to `sink`.
///
/// The two clocks meet here and nowhere else: the analyzer is advanced hop by
/// hop up to whatever has completed by the end of each output frame, and the
/// renderer is advanced by one `1/fps` step per output frame. A frame rate above
/// the hop rate repeats the last published [`AnalysisFrame`] rather than
/// interpolating one — the analyzer publishes on hop boundaries and inventing
/// values between them would put something in the picture that the DSP never
/// derived.
///
/// Returns how many frames were rendered. Nothing is retained: `sink` sees each
/// frame once, and this holds no images at all.
pub fn render_frames(
    r: &mut Renderer,
    name: &str,
    pcm: &[f32],
    format: AudioFormat,
    fps: Fps,
    sink: &mut dyn FnMut(u32, &CaptureImage) -> Result<(), String>,
) -> Result<u32, String> {
    let frames = frame_count(pcm.len(), format, fps)?;
    let mut analyzer = Analyzer::new(format).map_err(|e| format!("--render: {e}"))?;
    let hop_samples = hop_samples(format);
    let hops = total_hops(pcm.len(), format);

    let mut pushed = 0usize;
    // Silence until the first hop lands — the same state the analyzer itself is
    // in before its window has anything in it.
    let mut current = AnalysisFrame::default();

    r.capture_stream(
        name,
        frames,
        fps.dt(),
        &mut |index| {
            let due = hops_through(index, fps, format).min(hops);
            while pushed < due {
                let start = pushed * hop_samples;
                let hop = pcm.get(start..start + hop_samples).unwrap_or(&[]);
                analyzer.push_interleaved(hop);
                current = analyzer.take_frame();
                pushed += 1;
            }
            current
        },
        sink,
    )
    .map_err(|e| format!("render `{name}`: {e}"))?;
    Ok(frames)
}

/// Render the clip and write the stream to stdout.
///
/// **Everything human-readable goes to stderr**, because stdout is the frame
/// stream: a summary line printed the way every other mode prints one would be
/// eight bytes of garbage in the middle of the user's video.
pub fn run(
    presets: Vec<Preset>,
    source: &str,
    req: &RenderRequest,
    pcm: &[f32],
    format: AudioFormat,
    label: &str,
) -> Result<(), String> {
    let name = match (&req.preset, presets.as_slice()) {
        (Some(name), _) => name.clone(),
        (None, [only]) => only.name.clone(),
        (None, _) => {
            return Err("--render renders one preset: name it with --preset <name>".to_string());
        }
    };
    let frames = frame_count(pcm.len(), format, req.fps)?;
    eprintln!(
        "render: {name} over {label} — {frames} frames at {} fps, {}x{}, tier {} [{source}]",
        req.fps.as_header_field(),
        req.width,
        req.height,
        req.tier.as_str()
    );

    let mut r = super::renderer(req.width, req.height, presets, req.tier)?;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    out.write_all(y4m_header(req.width, req.height, req.fps).as_bytes())
        .map_err(|e| format!("write stream header: {e}"))?;

    let mut planes: Vec<u8> = Vec::new();
    let rendered = render_frames(&mut r, &name, pcm, format, req.fps, &mut |_index, img| {
        write_y4m_frame(img, &mut planes, &mut out).map_err(|e| format!("write frame: {e}"))
    })?;
    out.flush().map_err(|e| format!("flush stream: {e}"))?;

    eprintln!("render: {rendered} frames written");
    Ok(())
}

#[cfg(test)]
mod tests;
