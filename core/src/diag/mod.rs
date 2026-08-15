//! Runtime diagnostics: rolling frame-time statistics, core-tracked GPU-byte
//! accounting, and the flags that gate the on-screen overlay (Plan 0011).
//!
//! Two pieces live here, deliberately split so the math is testable without a
//! clock (NFR 6 determinism):
//!
//! - [`FrameStats`] is a **pure** accumulator: it is fed explicit frame deltas
//!   and computes fps / average / p99 from a fixed-capacity ring. No clock, no
//!   allocation — its unit tests carry no wall-clock read.
//! - [`Diag`] wraps it with the single **gated monotonic clock read** on the
//!   render path. That read is the one place `core` touches the wall clock; it
//!   is quarantined here (it never feeds DSP or scene animation, which stay a
//!   pure function of the input window + fixed step) and only runs while
//!   collection is enabled. See the risk note in Plan 0011.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to `diag` by Plan
// 0011). Runs every displayed frame; a panic here is a visible crash mid-show.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::time::Instant;

use crate::dsp::AnalysisFrame;

/// The downbeat estimator's per-beat decomposition, re-exported here because
/// **this** is the module a native shell reaches diagnostics through (Plan 0086
/// Phase 1).
///
/// It is defined beside the estimator it describes, and it stays there: nothing
/// about its shape is a diagnostics concern. What this line adds is the same
/// boundary statement [`AnalysisMetrics`] makes as a type — analysis diagnostics
/// are **native-only** (ADR-0052), so a consumer that finds them here has already
/// been told they do not cross the C ABI. Read it with
/// [`Analyzer::downbeat_terms`](crate::dsp::Analyzer::downbeat_terms).
pub use crate::dsp::downbeat::DownbeatTerms;

/// Recent frame durations retained for the rolling stats. 240 samples is ~4 s
/// at 60 fps — long enough for a stable p99, short enough to react to a stall.
///
/// **Lowering this is not a free tuning.** [`samples`](FrameStats::samples) is
/// what feeds the quality governor, so this value is also the longest series
/// [`sustained_miss`](crate::render::tier::sustained_miss) can ever be handed —
/// and below its
/// [`MIN_SAMPLES`](crate::render::tier::MIN_SAMPLES) the governor would stop
/// demoting entirely, silently. `pub(crate)` so that coupling is checked by a
/// compile-time assertion over there rather than left to whoever edits this line.
pub(crate) const RING: usize = 240;

/// A snapshot of the current diagnostics, mirroring the C ABI `LmvMetrics`
/// struct (ADR-0008) on the native side so both frontends surface identical
/// numbers from one computation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    /// Frames per second over the retained window.
    pub fps: f32,
    /// Mean frame time over the window, in milliseconds.
    pub frame_ms_avg: f32,
    /// 99th-percentile frame time over the window, in milliseconds.
    pub frame_ms_p99: f32,
    /// Frames recorded since creation (monotonic).
    pub frames_total: u64,
    /// Frames the renderer skipped (surface acquire returned nothing).
    pub frames_dropped: u64,
    /// Core-tracked GPU resource bytes — an approximation (wgpu does not report
    /// driver device memory), dominated by the swapchain. A trend indicator.
    pub gpu_bytes: u64,
    /// Draw/render-pass calls issued on the last frame.
    pub draw_calls: u32,
}

/// A snapshot of the **analysis** side of diagnostics: what the audio is doing,
/// as against [`Metrics`]'s what the renderer is doing.
///
/// # Why this is a second struct rather than six more fields on `Metrics`
///
/// [`Metrics`] claims, in its doc comment, to mirror the C ABI's `LmvMetrics`.
/// Growing it would either widen the C ABI or quietly make that claim false, and
/// ADR-0052 chose neither: these values stay **native-only**, so the split makes
/// "does not cross the ABI" a property of the type instead of a comment on a
/// field that a later plan has no reason to read. It follows the standing line
/// rather than inventing one — no analysis value has ever crossed the boundary
/// (`novelty` is already marked native-API-only on the frame itself).
///
/// The named cost, from the same ADR: the foobar plugin gets no analysis
/// diagnostics **programmatically** — no `lmv_get_metrics` counterpart, so
/// nothing on that path can compute a lock rate — and it is the one frontend
/// that never touches loopback capture. It does get them **on screen**: this
/// overlay is core-drawn, so a host setting `LMV_DEBUG_OVERLAY` paints the same
/// six rows the standalone's `F3` does (Plan 0049 close review corrected
/// ADR-0052 on that point; see its Outcome section). If the estimator ever needs
/// the machine-readable half on that path, `LmvMetrics` still leads with
/// `struct_size`, so the growth is available — behind a superseding ADR.
///
/// Exactly the six values Plan 0048 Phase 6 needs to make its two judgements:
/// whether the normalized levels ride the music, and whether the downbeat
/// estimator locks. Nothing speculative — the `*_raw` twins, `bpm`, `bar` and
/// `novelty` stay off until something asks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnalysisMetrics {
    /// Bass level, normalized against its recent peak (ADR-0049).
    pub bass: f32,
    /// Mid level, normalized against its recent peak.
    pub mid: f32,
    /// Treble level, normalized against its recent peak.
    pub treb: f32,
    /// Onset envelope, normalized against its recent peak.
    pub onset: f32,
    /// The downbeat estimator's confidence in `0..1` (ADR-0050).
    pub downbeat_confidence: f32,
    /// Whether the bar position came from the estimator rather than the counter
    /// fallback. **This is the value ADR-0050's stopping condition needs:** from
    /// outside the app a wrong lock and the fallback look identical.
    pub downbeat_locked: bool,
}

impl From<&AnalysisFrame> for AnalysisMetrics {
    fn from(frame: &AnalysisFrame) -> Self {
        Self {
            bass: frame.bass,
            mid: frame.mid,
            treb: frame.treb,
            onset: frame.onset,
            downbeat_confidence: frame.downbeat_confidence,
            downbeat_locked: frame.downbeat_locked,
        }
    }
}

/// Pure rolling frame-time accumulator. Fed explicit deltas (seconds); holds no
/// clock, so it is fully unit-testable. Fixed capacity — no per-frame alloc.
pub struct FrameStats {
    ring: [f32; RING],
    /// Next write position. Entries are written sequentially and wrap; the
    /// valid set is the first `len` slots (before wrap) or all of them (after),
    /// so `ring.iter().take(len)` always yields exactly the retained samples.
    head: usize,
    len: usize,
    frames_total: u64,
    frames_dropped: u64,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameStats {
    /// An empty accumulator.
    pub fn new() -> Self {
        Self {
            ring: [0.0; RING],
            head: 0,
            len: 0,
            frames_total: 0,
            frames_dropped: 0,
        }
    }

    /// Record one frame's duration (seconds) into the ring and bump the total.
    pub fn record(&mut self, dt_secs: f32) {
        if let Some(slot) = self.ring.get_mut(self.head) {
            *slot = dt_secs;
        }
        self.head = (self.head + 1) % RING;
        if self.len < RING {
            self.len += 1;
        }
        self.frames_total = self.frames_total.saturating_add(1);
    }

    /// Count a frame the renderer could not present (surface acquire skip).
    pub fn record_dropped(&mut self) {
        self.frames_dropped = self.frames_dropped.saturating_add(1);
    }

    /// Total sum of the retained durations (seconds).
    fn sum(&self) -> f32 {
        self.ring.iter().take(self.len).sum()
    }

    /// Frames per second over the retained window (0 until the first sample).
    pub fn fps(&self) -> f32 {
        let sum = self.sum();
        if self.len == 0 || sum <= 0.0 {
            return 0.0;
        }
        self.len as f32 / sum
    }

    /// Mean frame time over the window, in milliseconds.
    pub fn frame_ms_avg(&self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        self.sum() / self.len as f32 * 1000.0
    }

    /// 99th-percentile frame time over the window, in milliseconds. Copies the
    /// retained samples into a fixed local buffer and sorts — no allocation.
    pub fn frame_ms_p99(&self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let mut buf = [0.0f32; RING];
        let mut n = 0usize;
        for (dst, src) in buf.iter_mut().zip(self.ring.iter().take(self.len)) {
            *dst = *src;
            n += 1;
        }
        let Some(slice) = buf.get_mut(..n) else {
            return 0.0;
        };
        slice.sort_by(f32::total_cmp);
        // Nearest-rank on the retained samples: index round(0.99 * (n-1)).
        let idx = ((n - 1) as f32 * 0.99).round() as usize;
        slice.get(idx).copied().unwrap_or(0.0) * 1000.0
    }

    /// Frames recorded since creation.
    pub fn frames_total(&self) -> u64 {
        self.frames_total
    }

    /// Frames the renderer skipped.
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped
    }
}

/// The render-side diagnostics state: the pure [`FrameStats`], the gated clock,
/// the GPU-byte / draw-call figures, and the two flags that control collection
/// and the on-screen overlay.
pub struct Diag {
    /// While true, [`Diag::record_frame`] reads the monotonic clock and feeds
    /// the delta to `stats`. The standalone leaves this on so the title always
    /// shows live fps/p99; a host can turn it off to stay fully clock-free.
    collecting: bool,
    /// While true, the renderer paints the debug overlay as a final pass. Off by
    /// default — a live show is clean until a key/menu/env turns it on.
    overlay: bool,
    stats: FrameStats,
    /// Timestamp of the last presented frame; `None` after a toggle or a drop so
    /// the next delta is not inflated by the gap.
    last: Option<Instant>,
    gpu_bytes: u64,
    draw_calls: u32,
    /// The latest frame's analysis snapshot. Held here rather than recomputed on
    /// read so the overlay and the 1 Hz logger see the same six values, and so
    /// the logger's sampling stays a plain field read on the frames a line is
    /// due. Not gated by `collecting`: it holds no clock, and the overlay is
    /// independently toggled.
    analysis: AnalysisMetrics,
}

impl Default for Diag {
    fn default() -> Self {
        Self::new()
    }
}

impl Diag {
    /// A fresh diagnostics state: not collecting, overlay off.
    pub fn new() -> Self {
        Self {
            collecting: false,
            overlay: false,
            stats: FrameStats::new(),
            last: None,
            gpu_bytes: 0,
            draw_calls: 0,
            analysis: AnalysisMetrics::default(),
        }
    }

    /// Enable or disable rolling frame-time collection (the gated clock read).
    /// Toggling resets the delta chain so a re-enable does not record a spurious
    /// long frame across the disabled gap.
    pub fn set_collecting(&mut self, on: bool) {
        self.collecting = on;
        self.last = None;
    }

    /// Whether frame-time collection is currently enabled.
    pub fn collecting(&self) -> bool {
        self.collecting
    }

    /// Enable or disable painting the on-screen overlay.
    pub fn set_overlay(&mut self, on: bool) {
        self.overlay = on;
    }

    /// Whether the overlay should be painted this frame.
    pub fn overlay_enabled(&self) -> bool {
        self.overlay
    }

    /// Record a presented frame. Reads the monotonic clock (only while
    /// collecting) and feeds the delta since the previous present to the stats.
    pub fn record_frame(&mut self) {
        if !self.collecting {
            return;
        }
        // The one wall-clock read in `core`. Quarantined to diagnostics: it never
        // feeds DSP or scene animation (those stay a pure function of the input
        // window + fixed step), so NFR 6 determinism holds. See Plan 0011 risks.
        #[allow(
            clippy::disallowed_methods,
            reason = "diagnostics-only monotonic read, gated behind `collecting`, never feeds analysis or visual output (NFR 6 carve-out)"
        )]
        let now = Instant::now();
        if let Some(last) = self.last {
            self.stats
                .record(now.saturating_duration_since(last).as_secs_f32());
        }
        self.last = Some(now);
    }

    /// Count a frame the renderer could not present. Breaks the delta chain so
    /// the next present's delta excludes the skipped interval.
    pub fn record_dropped(&mut self) {
        if !self.collecting {
            return;
        }
        self.stats.record_dropped();
        self.last = None;
    }

    /// Set the core-tracked GPU resource byte estimate (fed from the render
    /// context each frame — dominated by the swapchain).
    pub fn set_gpu_bytes(&mut self, bytes: u64) {
        self.gpu_bytes = bytes;
    }

    /// Set the draw/render-pass count issued on the last frame.
    pub fn set_draw_calls(&mut self, n: u32) {
        self.draw_calls = n;
    }

    /// The current rolling snapshot.
    pub fn metrics(&self) -> Metrics {
        Metrics {
            fps: self.stats.fps(),
            frame_ms_avg: self.stats.frame_ms_avg(),
            frame_ms_p99: self.stats.frame_ms_p99(),
            frames_total: self.stats.frames_total(),
            frames_dropped: self.stats.frames_dropped(),
            gpu_bytes: self.gpu_bytes,
            draw_calls: self.draw_calls,
        }
    }

    /// Take this frame's analysis snapshot. Called once per drawn frame from the
    /// render seam, which is the one place that holds the [`AnalysisFrame`].
    pub fn set_analysis(&mut self, frame: &AnalysisFrame) {
        self.analysis = AnalysisMetrics::from(frame);
    }

    /// The latest analysis snapshot — the native-only companion to
    /// [`metrics`](Diag::metrics). See [`AnalysisMetrics`] for why it is separate.
    pub fn analysis(&self) -> AnalysisMetrics {
        self.analysis
    }

    /// Read-only view of the rolling stats (the overlay reads this to draw the
    /// frame-time sparkline).
    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }
}

impl FrameStats {
    /// The retained frame durations (seconds), oldest first, for the overlay's
    /// sparkline. Borrows the ring in chronological order without allocating.
    pub fn samples(&self) -> impl Iterator<Item = f32> + '_ {
        // Before wrap the valid slots are 0..len; after wrap they are the whole
        // ring starting at `head` (the oldest). Chain the two halves so the
        // iterator is chronological in both cases.
        let (full, head, len) = (self.len == RING, self.head, self.len);
        let (a, b) = if full {
            (self.ring.get(head..), self.ring.get(..head))
        } else {
            (self.ring.get(..len), None)
        };
        a.into_iter()
            .flatten()
            .chain(b.into_iter().flatten())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pure accumulator computes fps, average, and p99 from a known
    /// sequence with no clock in the loop (NFR 6 determinism).
    #[test]
    fn frame_stats_computes_known_sequence() {
        let mut stats = FrameStats::new();
        // 100 frames of 1..=100 ms. Sum = 5050 ms = 5.05 s.
        for ms in 1..=100u32 {
            stats.record(ms as f32 / 1000.0);
        }
        assert_eq!(stats.frames_total(), 100);

        // fps = 100 frames / 5.05 s = 19.8019...
        assert!(
            (stats.fps() - 19.801_98).abs() < 1e-2,
            "fps was {}",
            stats.fps()
        );
        // avg = mean(1..=100) ms = 50.5 ms.
        assert!(
            (stats.frame_ms_avg() - 50.5).abs() < 1e-3,
            "avg was {}",
            stats.frame_ms_avg()
        );
        // Sorted [1..=100]; nearest-rank index = round(0.99 * 99) = 98 → 99 ms.
        assert!(
            (stats.frame_ms_p99() - 99.0).abs() < 1e-3,
            "p99 was {}",
            stats.frame_ms_p99()
        );
    }

    /// An empty accumulator reports zeros, not NaN or a panic.
    #[test]
    fn frame_stats_empty_is_zero() {
        let stats = FrameStats::new();
        assert_eq!(stats.fps(), 0.0);
        assert_eq!(stats.frame_ms_avg(), 0.0);
        assert_eq!(stats.frame_ms_p99(), 0.0);
    }

    /// The ring retains only the most recent RING samples once it wraps.
    #[test]
    fn frame_stats_ring_wraps_to_recent() {
        let mut stats = FrameStats::new();
        // Fill with 5 ms, then overwrite the whole ring with 20 ms.
        for _ in 0..RING {
            stats.record(0.005);
        }
        for _ in 0..RING {
            stats.record(0.020);
        }
        assert_eq!(stats.frames_total(), (RING * 2) as u64);
        // Only the 20 ms samples remain → avg 20 ms, fps 50.
        assert!((stats.frame_ms_avg() - 20.0).abs() < 1e-3);
        assert!((stats.fps() - 50.0).abs() < 1e-2);
    }

    /// The six values are the frame's own, taken from a synthesized frame rather
    /// than eyeballed on screen — and the destructure is exhaustive, so a field
    /// added to `AnalysisMetrics` fails to compile until it is populated here.
    #[test]
    fn analysis_metrics_are_the_frames_own_values() {
        let frame = AnalysisFrame {
            bass: 0.25,
            mid: 0.5,
            treb: 0.75,
            onset: 0.125,
            downbeat_confidence: 0.875,
            downbeat_locked: true,
            // Values that must NOT leak into the six — the raw twins carry
            // deliberately different magnitudes so a mixed-up field is visible.
            bass_raw: 9.0,
            mid_raw: 9.0,
            treb_raw: 9.0,
            onset_raw: 9.0,
            ..Default::default()
        };

        let mut diag = Diag::new();
        assert_eq!(
            diag.analysis(),
            AnalysisMetrics::default(),
            "a fresh Diag reports zeros, not stale or uninitialized analysis"
        );
        diag.set_analysis(&frame);

        let AnalysisMetrics {
            bass,
            mid,
            treb,
            onset,
            downbeat_confidence,
            downbeat_locked,
        } = diag.analysis();
        assert_eq!(bass, 0.25);
        assert_eq!(mid, 0.5);
        assert_eq!(treb, 0.75);
        assert_eq!(onset, 0.125);
        assert_eq!(downbeat_confidence, 0.875);
        assert!(downbeat_locked);
    }

    /// ADR-0052 keeps `Metrics`'s "mirrors `LmvMetrics`" claim literally true by
    /// putting the analysis values on their own type. This exhaustive destructure
    /// is what enforces it: adding a field to `Metrics` stops compiling here, so a
    /// later plan has to come and read this comment before widening the mirror.
    /// `core/tests/ffi.rs` holds the other half — `LmvMetrics` is still 56 bytes
    /// and `LMV_ABI_VERSION` is still 4.
    #[test]
    fn metrics_still_carries_exactly_the_mirrored_fields() {
        let Metrics {
            fps,
            frame_ms_avg,
            frame_ms_p99,
            frames_total,
            frames_dropped,
            gpu_bytes,
            draw_calls,
        } = Diag::new().metrics();
        assert_eq!(
            (fps, frame_ms_avg, frame_ms_p99),
            (0.0, 0.0, 0.0),
            "a fresh Diag reports zero frame-time statistics"
        );
        assert_eq!((frames_total, frames_dropped, gpu_bytes), (0, 0, 0));
        assert_eq!(draw_calls, 0);
    }

    /// `samples()` yields the retained durations chronologically.
    #[test]
    fn samples_are_chronological_after_wrap() {
        let mut stats = FrameStats::new();
        for i in 0..(RING + 10) {
            stats.record(i as f32);
        }
        let got: Vec<f32> = stats.samples().collect();
        assert_eq!(got.len(), RING);
        // The oldest retained sample is (RING + 10) - RING = 10; last is RING+9.
        assert_eq!(got.first().copied(), Some(10.0));
        assert_eq!(got.last().copied(), Some((RING + 9) as f32));
    }
}
