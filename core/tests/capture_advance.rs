//! The capture path can advance the analyzer without rasterizing (Plan 0084
//! Phase 3).
//!
//! [`Renderer::capture_audio_after_warmup`] lets a caller feed hops that publish
//! their [`AnalysisFrame`](lmv_core::dsp::AnalysisFrame) but never reach a
//! render pass. The reactivity gate spends ~85 % of its frames on exactly that —
//! `WARMUP_HOPS` renders per capture, at silence, read back by nobody — and the
//! only thing that makes skipping them safe is that **analysis is a pure
//! function of its window** (`CLAUDE.md`): the render pass never touches the
//! analyzer, so the frames a warmed-up run publishes must be the frames a fully
//! rendered run published, bit for bit.
//!
//! That is asserted here rather than argued, and asserted on `to_bits` rather
//! than on `==`, because float equality would let a NaN pass as a difference and
//! `-0.0` pass as a match. The comparator is shown able to fail: the same clip
//! driven eight hops further publishes a different final frame.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, HOP_SIZE, WARMUP_HOPS};
use lmv_core::preset::default_presets;
use lmv_core::signal::click_track;

mod common;

/// Small offscreen size — nothing here reads a pixel, and the software adapter
/// is slow.
const SIZE: u32 = 64;

/// The format the reactivity gate runs at, so this property is asserted for the
/// caller that consumes it.
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// Hops rendered past the warm-up. Small on purpose: this test is about what the
/// analyzer publishes, not about a settled picture.
const SIGNAL_HOPS: usize = 6;
const HOPS: usize = WARMUP_HOPS + SIGNAL_HOPS;

/// A click track, because the property has to be checked on frames that
/// *change*: a steady tone's published frame can repeat hop to hop, which would
/// let the non-vacuity check below pass on a comparator that compares nothing.
const CLICK_BPM: f32 = 240.0;

fn clip(hops: usize) -> Vec<f32> {
    let secs = (hops * HOP_SIZE) as f32 / FORMAT.sample_rate as f32;
    click_track(CLICK_BPM, secs, FORMAT)
}

/// Every published field as its raw bit pattern, labelled so a mismatch names
/// the field rather than an index.
fn bits(f: &AnalysisFrame) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = f
        .spectrum
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("spectrum[{i}]"), v.to_bits() as u64))
        .collect();
    let scalars: [(&str, f32); 12] = [
        ("onset", f.onset),
        ("bass", f.bass),
        ("mid", f.mid),
        ("treb", f.treb),
        ("bass_raw", f.bass_raw),
        ("mid_raw", f.mid_raw),
        ("treb_raw", f.treb_raw),
        ("onset_raw", f.onset_raw),
        ("bpm", f.bpm),
        ("bar", f.bar),
        ("time_since_beat", f.time_since_beat),
        ("bar_phase", f.bar_phase),
    ];
    for (name, v) in scalars {
        out.push((name.to_string(), v.to_bits() as u64));
    }
    out.push((
        "downbeat_confidence".into(),
        f.downbeat_confidence.to_bits() as u64,
    ));
    out.push(("novelty".into(), f.novelty.to_bits() as u64));
    out.push(("beat".into(), u64::from(f.beat)));
    out.push(("downbeat_locked".into(), u64::from(f.downbeat_locked)));
    out.push(("beat_index".into(), u64::from(f.beat_index)));
    out.push(("beat_in_bar".into(), u64::from(f.beat_in_bar)));
    out.push(("bar_index".into(), u64::from(f.bar_index)));
    out
}

/// The first field that differs, or `None` when the two frames are bit-identical.
fn first_difference(a: &AnalysisFrame, b: &AnalysisFrame) -> Option<String> {
    bits(a)
        .into_iter()
        .zip(bits(b))
        .find(|((_, x), (_, y))| x != y)
        .map(|((name, x), (_, y))| format!("{name}: {x:#x} vs {y:#x}"))
}

fn a_preset() -> String {
    default_presets()
        .first()
        .expect("the embedded set is never empty")
        .name
        .clone()
}

/// Feeding hops without rendering them publishes exactly the frames rendering
/// them published — every hop, every field, bit for bit.
#[test]
fn skipping_the_render_leaves_the_published_frames_identical() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();
    let pcm = clip(HOPS);
    let measured: Vec<u32> = (WARMUP_HOPS as u32..HOPS as u32).collect();

    let rendered = renderer
        .capture_audio_after_warmup(&name, &pcm, FORMAT, &measured, 0)
        .expect("capture with every hop rendered");
    let warmed = renderer
        .capture_audio_after_warmup(&name, &pcm, FORMAT, &measured, WARMUP_HOPS)
        .expect("capture with the warm-up hops advanced only");

    assert_eq!(
        rendered.analysis.len(),
        HOPS,
        "one published frame per hop, whether or not it was drawn"
    );
    assert_eq!(warmed.analysis.len(), rendered.analysis.len());

    for (hop, (a, b)) in rendered
        .analysis
        .iter()
        .zip(warmed.analysis.iter())
        .enumerate()
    {
        if let Some(diff) = first_difference(a, b) {
            panic!("hop {hop} published a different frame without the render: {diff}");
        }
    }

    // The frame the gate actually reads first is the one right after the
    // warm-up, so it is called out rather than left implicit in the loop.
    let next = WARMUP_HOPS;
    assert!(
        first_difference(&rendered.analysis[next], &warmed.analysis[next]).is_none(),
        "the first frame past the warm-up is where a divergence would land"
    );
}

/// The comparator above can fail. Eight more hops of the same clip leave the
/// analyzer somewhere else, and `first_difference` says so — without this, a
/// comparator that compared nothing would pass the test above.
#[test]
fn the_bit_comparison_can_fail() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();

    let short = renderer
        .capture_audio_after_warmup(&name, &clip(HOPS), FORMAT, &[], HOPS)
        .expect("advance the short clip");
    let long = renderer
        .capture_audio_after_warmup(&name, &clip(HOPS + 8), FORMAT, &[], HOPS + 8)
        .expect("advance the long clip");

    assert_eq!(short.analysis.len(), HOPS);
    assert_eq!(long.analysis.len(), HOPS + 8);
    let a = short.analysis.last().expect("a frame per hop");
    let b = long.analysis.last().expect("a frame per hop");
    assert!(
        first_difference(a, b).is_some(),
        "a different hop count must publish a different final frame, or the \
         equality above is proving nothing"
    );
}

/// A run that is all warm-up rasterizes nothing, counted rather than timed.
#[test]
fn an_all_warmup_run_renders_zero_frames() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();
    let pcm = clip(HOPS);

    let none = renderer
        .capture_audio_after_warmup(&name, &pcm, FORMAT, &[], HOPS)
        .expect("advance without rendering");
    assert_eq!(
        none.rendered, 0,
        "no hop past the warm-up, so no render pass"
    );
    assert_eq!(none.analysis.len(), HOPS, "the analyzer still ran");

    let all = renderer
        .capture_audio_after_warmup(&name, &pcm, FORMAT, &[], 0)
        .expect("render every hop");
    assert_eq!(all.rendered, HOPS, "the old behaviour, unchanged");

    let gate_shaped = renderer
        .capture_audio_after_warmup(
            &name,
            &pcm,
            FORMAT,
            &(WARMUP_HOPS as u32..HOPS as u32).collect::<Vec<_>>(),
            WARMUP_HOPS,
        )
        .expect("the shape core/tests/reactivity.rs uses");
    assert_eq!(
        gate_shaped.rendered, SIGNAL_HOPS,
        "the measured window and nothing else"
    );
    assert_eq!(gate_shaped.images.len(), SIGNAL_HOPS);
}

// ---------------------------------------------------------------------------
// The long-run primitive (Plan 0085 Phase 1)
// ---------------------------------------------------------------------------

/// Frame indices the horizon cases sample at. Small — the claims here are about
/// *which* frame comes back, not about a settled picture.
const SAMPLES: [u32; 3] = [3, 9, 17];

/// `capture_preset_at` is `capture_preset` continued, not a second renderer.
///
/// The whole value of a horizon row is that it can be compared with every other
/// capture this repo takes — a `--report` column, a golden baseline, an author's
/// `shot`. That holds only if sampling frame `n - 1` of a long run returns the
/// same pixels `capture_preset(name, frame, n)` returns, which is asserted here
/// on the bytes rather than argued from the two functions looking alike. The
/// software adapter is the strict case: it is the one where *when* a GPU buffer
/// is allocated changes what the feedback stages resolve to, which is why the
/// readback buffer is built at the first sample rather than up front.
#[test]
fn a_sampled_frame_is_the_frame_a_plain_capture_of_that_length_returns() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();
    let stimulus = AnalysisFrame::default();

    for n in SAMPLES {
        let sampled = renderer
            .capture_preset_at(&name, &stimulus, &[n])
            .expect("sample one frame of a run");
        let plain = renderer
            .capture_preset(&name, &stimulus, n + 1)
            .expect("the same length as a plain capture");
        let sampled = sampled.first().expect("one requested frame, one image");
        assert_eq!(
            (sampled.width, sampled.height),
            (plain.width, plain.height),
            "frame {n}: capture dimensions differ"
        );
        assert!(
            sampled.rgba == plain.rgba,
            "frame {n} of a sampled run differs from capture_preset(.., {}) — the \
             two paths have diverged, so a horizon row cannot be compared with \
             any other capture",
            n + 1
        );
    }
}

/// A row does not depend on how far the run was asked to go, and the images are
/// not all the same image.
///
/// These are the two properties a drift series rests on. The first is what lets
/// a two-minute run and a ten-minute run be read against each other — asserted
/// by taking the *same* early index alone and inside a longer request. The
/// second is the non-vacuity half: without it, a primitive that returned the
/// first frame for every index would satisfy everything above.
#[test]
fn an_earlier_row_is_unchanged_by_asking_for_a_longer_run() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();
    let stimulus = AnalysisFrame::default();
    let first = *SAMPLES.first().expect("SAMPLES is not empty");

    let short = renderer
        .capture_preset_at(&name, &stimulus, &[first])
        .expect("the short run");
    let long = renderer
        .capture_preset_at(&name, &stimulus, &SAMPLES)
        .expect("the long run");
    assert_eq!(long.len(), SAMPLES.len(), "one image per requested index");
    assert!(
        short.first().map(|i| &i.rgba) == long.first().map(|i| &i.rgba),
        "the first row changed when a longer horizon was requested — a drift \
         series read at two horizons would not be comparable"
    );

    // Non-vacuity: the last sampled frame is not the first one. A time-driven
    // scene has moved 14 frames on by then; if this ever fails the subject
    // preset has gone static, not the primitive.
    assert!(
        long.first().map(|i| &i.rgba) != long.last().map(|i| &i.rgba),
        "every sampled index returned the same pixels, so the equality above is \
         proving nothing (is `{name}` animating?)"
    );
}

/// The degenerate requests, which a horizon of zero intervals will reach.
#[test]
fn an_empty_request_renders_nothing_and_a_repeated_index_renders_once() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = a_preset();
    let stimulus = AnalysisFrame::default();

    assert!(
        renderer
            .capture_preset_at(&name, &stimulus, &[])
            .expect("an empty request is not an error")
            .is_empty(),
        "no requested frames, no images"
    );

    // A repeated index yields the same frame twice — the run is not re-rendered
    // and the caller's order is preserved.
    let repeated = renderer
        .capture_preset_at(&name, &stimulus, &[5, 2, 5])
        .expect("out-of-order and repeated indices");
    assert_eq!(repeated.len(), 3);
    assert!(
        repeated.first().map(|i| &i.rgba) == repeated.get(2).map(|i| &i.rgba),
        "the same index must come back as the same image"
    );
    assert!(
        repeated.first().map(|i| &i.rgba) != repeated.get(1).map(|i| &i.rgba),
        "frames 5 and 2 came back identical"
    );

    assert!(
        renderer
            .capture_preset_at("definitely not a preset", &stimulus, &[1])
            .is_err(),
        "an unknown preset is an error, as it is for every capture entry point"
    );
}
