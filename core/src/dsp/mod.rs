//! Deterministic analysis of the PCM stream: windowed FFT spectrum plus an
//! onset envelope and beat flag, delivered once per hop as an
//! [`AnalysisFrame`].
//!
//! Everything here is a pure function of the samples fed in — no wall clock,
//! no unseeded randomness (NFR section 6). Window and hop sizes fit the 60 ms
//! latency budget at 48 kHz: one hop is ~10.7 ms (NFR section 3).
//!
//! Two FFT windows feed one band axis (ADR-0049): [`WINDOW_SIZE`] carries
//! everything it can resolve — including all of onset, beat and tempo, so the
//! transient path keeps its speed — and [`LOW_WINDOW_SIZE`] carries the bands
//! below the crossover, which a 23 kHz-wide bin cannot. See [`fft::BandLayout`].

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Analysis runs every hop
// off the render loop; it must never panic on valid input.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod bands;
pub mod downbeat;
pub mod fft;
pub mod gain;
pub mod grid;
pub mod novelty;
pub mod onset;
pub mod tempo;

use crate::audio::{AudioFormat, FormatError};

/// FFT window length in samples (~43 ms at 48 kHz).
pub const WINDOW_SIZE: usize = 2048;

/// How many time-domain samples an [`AnalysisFrame`] carries in
/// [`waveform`](AnalysisFrame::waveform).
///
/// **512, which is MilkDrop's own count** (Plan 0100 Phase 4): its waveform draws
/// 512 consecutive samples, every preset's `wave_mode` geometry is written
/// against that resolution, and a converted preset should draw the figure its
/// author drew. At 48 kHz it is 10.7 ms of audio against MilkDrop's 11.6 ms at
/// 44.1 kHz — the same gesture, a fraction of a beat either way.
///
/// The samples are the **most recent** 512 of [`WINDOW_SIZE`], taken
/// consecutively rather than decimated across the whole window. Decimation would
/// alias: a 4:1 pick of every fourth sample of a 12 kHz tone at 48 kHz reads as a
/// 3 kHz one, and a waveform display exists to show exactly that shape.
pub const WAVE_SAMPLES: usize = 512;

// The tail this is taken from has to exist. A compile-time check rather than a
// runtime one, because the failure would be a silently shorter waveform.
const _: () = assert!(
    WAVE_SAMPLES <= WINDOW_SIZE,
    "the waveform is the tail of the short analysis window and cannot be longer      than it"
);
/// Second, longer FFT window feeding the bands below the crossover (~171 ms at
/// 48 kHz). Chosen by measurement in Plan 0048 Phase 1 against the plan's rule
/// — 4096 first, 8192 only if 4096 still leaves sub-bass bands bin-starved. Of
/// the 20 bands below the crossover, 4096 leaves **all 20** still one bin wide
/// and 8192 leaves 8, pulling the unresolved boundary down from 246 Hz to 76 Hz.
/// See [`fft::BandLayout`] and ADR-0049;
/// `the_long_window_was_chosen_by_measurement` pins all three candidates.
pub const LOW_WINDOW_SIZE: usize = 8192;
/// Samples between successive analysis hops (~10.7 ms at 48 kHz).
pub const HOP_SIZE: usize = 512;
/// Hops the analyzer consumes before it publishes its first frame — the time the
/// **longer** window takes to fill (~171 ms at 48 kHz).
///
/// Exported so callers that sample a clip "past warm-up" derive the offset
/// instead of restating it as a literal. Two already did, and both were silently
/// wrong the moment [`LOW_WINDOW_SIZE`] arrived.
pub const WARMUP_HOPS: usize = LOW_WINDOW_SIZE / HOP_SIZE;
/// Log-frequency bands exposed to scenes.
pub const SPECTRUM_BINS: usize = 64;

/// One hop's worth of analysis.
///
/// **The four headline levels are normalized** (ADR-0049): `bass`, `mid`, `treb`
/// and `onset` are each a 0..1 fraction of that signal's own slowly-decaying
/// recent peak, so `> 0.5` means "loud for this track" rather than naming an
/// absolute magnitude that depended on the gain staging. The absolute values
/// remain as `*_raw` for looks that genuinely want them, and for harness
/// continuity. `spectrum` normalizes against **one** peak shared by the whole
/// array, so every ratio inside it — and therefore every `bin()` contrast — comes
/// through untouched; see [`gain::BandNormalizer`] for why per-band would not.
///
/// `beat` flags an onset event this hop; `bpm`/`bar` come from the tempo tracker,
/// which reads the **raw** onset — see [`gain`] for why the internal consumers
/// are deliberately left on raw values. The bar-position trio comes from
/// [`downbeat`], and falls back to plain counters whenever its estimate is not
/// confident (ADR-0050).
#[derive(Debug, Clone, Copy)]
pub struct AnalysisFrame {
    /// Per-band energy, the whole array normalized against **one shared** recent
    /// peak — so every ratio inside it, and therefore every `bin()` contrast,
    /// comes through untouched. Not a per-band normalization: that was the draft
    /// ADR-0049 rejected, because it flattens the very shape a spectrum is for.
    pub spectrum: [f32; SPECTRUM_BINS],
    /// The most recent [`WAVE_SAMPLES`] of the mono signal, in **time** order —
    /// the oscilloscope trace, not a spectrum (Plan 0100 Phase 4 / ADR-0113).
    ///
    /// Nothing in the engine's own vocabulary reads this: the expression grammar
    /// is scalar and reaches the band array through `bin()` alone (ADR-0036), and
    /// widening it to an array type is exactly what ADR-0002's purity refuses.
    /// **It is here for the one consumer that genuinely needs a waveform** — the
    /// warp mesh's `wave_mode` draw, which is what MilkDrop's presets use as
    /// their light source, and which no amount of spectrum can reconstruct.
    ///
    /// Raw amplitude in roughly `-1..1`, un-normalized. The four headline levels
    /// are peak-normalized (ADR-0049) because a *threshold* on them has to be
    /// portable; a waveform is a picture of the signal, and normalizing it would
    /// make a quiet passage draw the same trace as a loud one — which is the
    /// opposite of what a scope is for. The consumer scales it (MilkDrop's
    /// `wave_scale` does exactly that).
    ///
    /// **This is the one array on this struct that is not normalized and the one
    /// that made it big.** `AnalysisFrame` is `Copy` and copied per frame, and 512
    /// floats take it from ~340 bytes to ~2.4 kB — about 100 ns of memcpy at
    /// 60 Hz, which is why it was acceptable. It is deliberately **not** in
    /// [`Variables`](crate::preset::Variables), which carries the band array by
    /// borrow for precisely this reason.
    pub waveform: [f32; WAVE_SAMPLES],
    /// Spectral-flux onset envelope, normalized against its recent peak.
    pub onset: f32,
    /// Whether a beat (onset event) fired this hop.
    pub beat: bool,
    /// Bass-band level (~20-250 Hz), normalized against its recent peak.
    pub bass: f32,
    /// Mid-band level (~250-4000 Hz), normalized against its recent peak.
    pub mid: f32,
    /// Treble-band level (~4-18 kHz), normalized against its recent peak.
    pub treb: f32,
    /// Raw mean magnitude in the bass band — the pre-ADR-0049 `bass`, unchanged.
    pub bass_raw: f32,
    /// Raw mean magnitude in the mid band — the pre-ADR-0049 `mid`, unchanged.
    pub mid_raw: f32,
    /// Raw mean magnitude in the treble band — the pre-ADR-0049 `treb`, unchanged.
    pub treb_raw: f32,
    /// Raw spectral-flux envelope — the pre-ADR-0049 `onset`, unchanged.
    pub onset_raw: f32,
    /// Tempo estimate in BPM (hop-clock autocorrelation; 0 until warm).
    pub bpm: f32,
    /// Beat phase in [0, 1): 0 on each beat, ramping to the next.
    ///
    /// The name is a **documented misnomer** — this is beat phase, not bar phase.
    /// Too widely bound to rename (ADR-0050); `bar_phase` is the true quantity.
    pub bar: f32,
    /// Monotone count of beats since the stream started, 0 on the first beat
    /// (ADR-0050 Layer 1). Unconditional and deterministic — no confidence gate.
    pub beat_index: u32,
    /// Seconds since the last detected beat; exactly 0 on a beat hop.
    pub time_since_beat: f32,
    /// Which beat of the bar this is, `0..4` (ADR-0050 Layer 2). Estimated when
    /// the downbeat tracker is confident, `beat_index % 4` otherwise.
    pub beat_in_bar: u32,
    /// Bar counter, on the same gated-or-counted basis. **Monotone except across
    /// an alignment change** — it is `(beat_index - alignment) / 4`, so the beat
    /// the estimator locks, drops back, or moves its alignment can repeat or skip
    /// a bar. Hysteresis makes that rare (a challenger must lead for three bars),
    /// and a repeated bar is a far softer failure than a wrong downbeat — but
    /// `mod(bar_index, 8)` will see it.
    pub bar_index: u32,
    /// Position across the bar in `[0, 1)` — the true bar phase, as against
    /// [`bar`](Self::bar), which is beat phase under a historical name.
    pub bar_phase: f32,
    /// Downbeat-alignment confidence in `0..1`. **Diagnostics only** — not a
    /// grammar variable, so authors get behavior rather than homework.
    pub downbeat_confidence: f32,
    /// Whether the bar trio above came from the estimator rather than the
    /// counter fallback. **Diagnostics only**, as with the confidence.
    pub downbeat_locked: bool,
    /// Experimental spectral track-change novelty (Plan 0009 Phase 4): ~0 within
    /// a steady segment, spiking at a spectral boundary. Native-API only — not
    /// exposed across the C ABI.
    pub novelty: f32,
}

impl Default for AnalysisFrame {
    fn default() -> Self {
        Self {
            spectrum: [0.0; SPECTRUM_BINS],
            waveform: [0.0; WAVE_SAMPLES],
            onset: 0.0,
            beat: false,
            bass: 0.0,
            mid: 0.0,
            treb: 0.0,
            bass_raw: 0.0,
            mid_raw: 0.0,
            treb_raw: 0.0,
            onset_raw: 0.0,
            bpm: 0.0,
            bar: 0.0,
            beat_index: 0,
            time_since_beat: 0.0,
            beat_in_bar: 0,
            bar_index: 0,
            bar_phase: 0.0,
            downbeat_confidence: 0.0,
            downbeat_locked: false,
            novelty: 0.0,
        }
    }
}

/// Stateful per-stream analyzer: accumulates interleaved samples into mono
/// hops, runs FFT + onset detection each completed hop, and hands the latest
/// frame to the render side. Deterministic for a given sample sequence.
///
/// After construction, processing allocates nothing — safe to drive from the
/// render loop every frame.
pub struct Analyzer {
    format: AudioFormat,
    spectrum: fft::SpectrumAnalyzer,
    onset: onset::OnsetDetector,
    bands: bands::BandSplitter,
    tempo: tempo::TempoTracker,
    /// Layer 2's own beat clock (ADR-0109), driven by the tempo estimate rather
    /// than by the transient stream. Nothing outside [`Self::push_interleaved`]
    /// reads it and it publishes no grammar variable — it exists so the downbeat
    /// fold has a unit that is a beat.
    grid: grid::BarGrid,
    novelty: novelty::NoveltyDetector,
    /// ADR-0050 Layer 2. Reads the **normalized** bass and flux, unlike the
    /// detectors above: its accent blend weighs the two against each other, which
    /// is only meaningful once both are on a common 0..1 scale.
    downbeat: downbeat::DownbeatTracker,
    /// Published-surface normalizers (ADR-0049). Deliberately *after* the
    /// detectors above in the hop, so each of those keeps reading raw values.
    band_gain: gain::BandNormalizer,
    bass_gain: gain::PeakNormalizer,
    mid_gain: gain::PeakNormalizer,
    treb_gain: gain::PeakNormalizer,
    onset_gain: gain::PeakNormalizer,
    window: [f32; WINDOW_SIZE],
    /// The long window feeding the sub-crossover bands (ADR-0049). Heap-held:
    /// 32 KB, and `Analyzer` is moved by value.
    low_window: Vec<f32>,
    /// Samples seen, saturating at [`LOW_WINDOW_SIZE`]. Analysis waits for the
    /// **longer** window, so no frame is ever published from a partly-filled
    /// one: Hann weights the newest samples near zero, so a half-full long
    /// window reads its low bands *low* and ramps as real audio reaches the
    /// taper's centre. That ramp is a genuine spectral transient — the novelty
    /// detector's 2 s running mean integrates it into seconds of spurious
    /// score, which would nudge the scene director at every stream start. The
    /// cost is first analysis at ~171 ms instead of ~43 ms, at cold start only;
    /// NFR section 3's beat-to-reaction budget is about steady state and does
    /// not move.
    filled: usize,
    hop: [f32; HOP_SIZE],
    hop_filled: usize,
    latest: AnalysisFrame,
    /// Beats are sticky between `take_frame` calls so a beat can never fall
    /// between two render frames and vanish.
    pending_beat: bool,
}

impl Analyzer {
    /// Build an analyzer for a validated stream format.
    pub fn new(format: AudioFormat) -> Result<Self, FormatError> {
        let format = format.validate()?;
        Ok(Self {
            format,
            spectrum: fft::SpectrumAnalyzer::new(format.sample_rate),
            onset: onset::OnsetDetector::new(),
            bands: bands::BandSplitter::new(format.sample_rate),
            tempo: tempo::TempoTracker::new(format.sample_rate),
            grid: grid::BarGrid::new(format.sample_rate),
            novelty: novelty::NoveltyDetector::new(format.sample_rate),
            downbeat: downbeat::DownbeatTracker::new(),
            band_gain: gain::BandNormalizer::new(format.sample_rate),
            bass_gain: gain::PeakNormalizer::new(format.sample_rate, gain::BAND_FLOOR),
            mid_gain: gain::PeakNormalizer::new(format.sample_rate, gain::BAND_FLOOR),
            treb_gain: gain::PeakNormalizer::new(format.sample_rate, gain::BAND_FLOOR),
            onset_gain: gain::PeakNormalizer::new(format.sample_rate, gain::ONSET_FLOOR),
            window: [0.0; WINDOW_SIZE],
            low_window: vec![0.0; LOW_WINDOW_SIZE],
            filled: 0,
            hop: [0.0; HOP_SIZE],
            hop_filled: 0,
            latest: AnalysisFrame::default(),
            pending_beat: false,
        })
    }

    /// The validated format this analyzer was created with.
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// The log-frequency band a given frequency falls into — lets scenes and
    /// tests reason about where energy should show up.
    pub fn band_for_freq(&self, hz: f32) -> usize {
        self.spectrum.band_for_freq(hz)
    }

    /// Feed interleaved samples (whole frames, as produced by the intake).
    /// Runs one analysis pass per completed hop.
    #[allow(
        clippy::indexing_slicing,
        reason = "hop_filled < HOP_SIZE (reset at the boundary); both window tail slices are fixed (SIZE - HOP_SIZE) ranges of buffers allocated at exactly SIZE, so all are in-bounds by construction"
    )]
    pub fn push_interleaved(&mut self, samples: &[f32]) {
        let channels = self.format.channels as usize;
        for frame in samples.chunks_exact(channels) {
            let mono = frame.iter().sum::<f32>() / channels as f32;
            self.hop[self.hop_filled] = mono;
            self.hop_filled += 1;
            if self.hop_filled == HOP_SIZE {
                self.hop_filled = 0;
                self.window.copy_within(HOP_SIZE.., 0);
                self.window[WINDOW_SIZE - HOP_SIZE..].copy_from_slice(&self.hop);
                self.low_window.copy_within(HOP_SIZE.., 0);
                self.low_window[LOW_WINDOW_SIZE - HOP_SIZE..].copy_from_slice(&self.hop);
                self.filled = (self.filled + HOP_SIZE).min(LOW_WINDOW_SIZE);
                if self.filled == LOW_WINDOW_SIZE {
                    let raw_spectrum = self.spectrum.analyze(&self.window, &self.low_window);
                    let (onset_raw, beat) = self.onset.process(self.spectrum.magnitudes());
                    let (bass_raw, mid_raw, treb_raw) =
                        self.bands.split(self.spectrum.magnitudes());

                    // Every consumer below this line reads RAW values on purpose
                    // (see `gain`'s module docs): the tempo tracker
                    // autocorrelates the onset envelope, and peak-normalizing it
                    // would distort the periodicity it looks for, while novelty
                    // measures spectral shape, which per-band normalization
                    // flattens by construction.
                    let clock = self.tempo.process(onset_raw, beat);
                    let grid = self.grid.process(clock.bpm, onset_raw);
                    let novelty = self.novelty.process(&raw_spectrum);

                    // ...and normalization happens last, on the way out.
                    let mut spectrum = raw_spectrum;
                    self.band_gain.normalize(&mut spectrum);
                    let onset = self.onset_gain.normalize(onset_raw);
                    let bass = self.bass_gain.normalize(bass_raw);

                    // The downbeat tracker sits after normalization on purpose —
                    // it weighs bass against flux, which needs a common scale.
                    //
                    // What it folds over is the **grid's** beat count, not
                    // `beat_index` (ADR-0109): the latter counts transients, at
                    // 1.35x-2.10x per musical beat, so `beat_index % 4` spanned
                    // well under a bar and a bar-locked accent precessed across
                    // all four alignments. Until the grid is running — the tempo
                    // tracker needs its envelope history filled first — the old
                    // pair is passed, which is the counter fallback ADR-0050
                    // specifies rather than a second code path.
                    let (fold_count, fold_phase) = if grid.running {
                        (
                            grid.bar_index * downbeat::BEATS_PER_BAR + grid.beat_in_bar,
                            grid.beat_phase,
                        )
                    } else {
                        (clock.beat_index, clock.bar)
                    };
                    let bars = self
                        .downbeat
                        .process(beat, fold_count, bass, onset, fold_phase);

                    // The oscilloscope trace: the most recent `WAVE_SAMPLES` of
                    // the window, consecutive and un-normalized (Plan 0100
                    // Phase 4). A slice of a buffer the analyzer already holds —
                    // no extra state, no extra pass, and nothing here reads a
                    // clock, so the frame stays a pure function of its window.
                    let mut waveform = [0.0f32; WAVE_SAMPLES];
                    if let Some(tail) = self.window.get(WINDOW_SIZE - WAVE_SAMPLES..) {
                        waveform.copy_from_slice(tail);
                    }

                    self.latest = AnalysisFrame {
                        spectrum,
                        waveform,
                        onset,
                        beat,
                        bass,
                        mid: self.mid_gain.normalize(mid_raw),
                        treb: self.treb_gain.normalize(treb_raw),
                        bass_raw,
                        mid_raw,
                        treb_raw,
                        onset_raw,
                        bpm: clock.bpm,
                        bar: clock.bar,
                        beat_index: clock.beat_index,
                        time_since_beat: clock.time_since_beat,
                        beat_in_bar: bars.beat_in_bar,
                        bar_index: bars.bar_index,
                        bar_phase: bars.bar_phase,
                        downbeat_confidence: bars.confidence,
                        downbeat_locked: bars.locked,
                        novelty,
                    };
                    self.pending_beat |= beat;
                }
            }
        }
    }

    /// The downbeat estimator's current decomposition — Plan 0068's instrument,
    /// reachable from a native shell (Plan 0086 Phase 1).
    ///
    /// **Reading it changes nothing.** [`downbeat::DownbeatTracker::terms`] takes
    /// `&self`, recomputes from state [`push_interleaved`](Self::push_interleaved)
    /// already keeps, allocates nothing and reads no clock — so the estimator
    /// behaves identically whether or not anyone is looking, and the value is the
    /// published [`AnalysisFrame::downbeat_confidence`] bit for bit between hops.
    ///
    /// Diagnostics only, and **native-only** (ADR-0052): not a grammar variable,
    /// and never on the C ABI.
    pub fn downbeat_terms(&self) -> downbeat::DownbeatTerms {
        self.downbeat.terms()
    }

    /// Latest analysis with any beat since the previous take. Call once per
    /// render frame.
    pub fn take_frame(&mut self) -> AnalysisFrame {
        let mut frame = self.latest;
        frame.beat = self.pending_beat;
        self.pending_beat = false;
        self.latest.beat = false;
        frame
    }
}
