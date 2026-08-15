// Test bodies index and unwrap freely — not the hot path.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]

use super::*;

/// A run with hand-written rows, so the presentations are tested against known
/// numbers rather than against whatever a GPU produced.
fn a_run(samples: Vec<HorizonSample>) -> HorizonRun {
    HorizonRun {
        preset: "Subject".to_string(),
        tier: Tier::Floor,
        width: 96,
        height: 96,
        minutes: 1.0,
        interval_secs: 30.0,
        samples,
        cost: RunCost {
            frames: 3600,
            wall_secs: 12.5,
            rss_before_bytes: Some(400 * 1024 * 1024),
            rss_after_bytes: Some(410 * 1024 * 1024),
            software_adapter: false,
        },
    }
}

fn sample(frame: u32, coverage: f32, peak_to_mean: f32, motion: Option<f32>) -> HorizonSample {
    HorizonSample {
        frame,
        elapsed_secs: elapsed_secs(frame),
        coverage,
        footprint_diff: motion,
        peak_to_mean,
    }
}

/// The property that makes two runs of different lengths comparable: row *k*
/// sits at the same frame index whatever horizon was requested.
///
/// This is one of the two determinism claims Plan 0085 Phase 1 asserts rather
/// than assumes, and it is the half that can be settled without a GPU — the
/// other (the same request rendering the same pixels twice) lives in
/// `standalone/tests/shot_cli.rs`.
#[test]
fn a_rows_frame_index_does_not_depend_on_the_horizon_requested() {
    let short = sample_frames(2.0, 30.0).expect("a 2-minute horizon");
    let long = sample_frames(10.0, 30.0).expect("a 10-minute horizon");

    // 2 minutes at 30 s is four intervals, plus the reference row at frame 0.
    assert_eq!(short, vec![0, 1800, 3600, 5400, 7200]);
    assert_eq!(long.len(), 21, "10 minutes at 30 s is twenty intervals");
    assert!(
        long.starts_with(&short),
        "the long run must agree with the short one on every shared index: \
         {short:?} vs {long:?}"
    );

    // ...and the interval divides the indices exactly, at any interval length.
    for (minutes, interval, step) in [(1.0, 60.0, 3600), (5.0, 15.0, 900), (0.05, 1.0, 60)] {
        let frames = sample_frames(minutes, interval).expect("a valid request");
        assert!(
            frames
                .iter()
                .enumerate()
                .all(|(k, f)| *f == k as u32 * step),
            "{minutes}min/{interval}s did not land on multiples of {step}: {frames:?}"
        );
    }
}

#[test]
fn a_horizon_that_cannot_be_sampled_is_an_error_naming_the_flag() {
    for (minutes, interval, needle) in [
        (0.0, 30.0, "--horizon"),
        (-1.0, 30.0, "--horizon"),
        (f32::NAN, 30.0, "--horizon"),
        (MAX_MINUTES + 1.0, 30.0, "ceiling"),
        (1.0, 0.0, "--interval"),
        (1.0, -5.0, "--interval"),
        (1.0, f32::INFINITY, "--interval"),
        // A single interval longer than the whole run samples nothing, which is
        // a request that cannot be honoured rather than an empty table.
        (1.0, 90.0, "longer than"),
        // Under one frame at the capture rate.
        (1.0, 0.001, "under one frame"),
    ] {
        let err = sample_frames(minutes, interval)
            .expect_err("{minutes}min/{interval}s must be rejected");
        assert!(
            err.contains(needle),
            "{minutes}min/{interval}s: error does not name `{needle}`: {err}"
        );
    }

    // The smallest honourable request is one interval, which is two rows.
    assert_eq!(sample_frames(1.0, 60.0).unwrap(), vec![0, 3600]);
}

#[test]
fn elapsed_time_is_simulated_frames_over_the_capture_rate() {
    // The clock advances before each frame is drawn, so frame 0 is one step in.
    assert!((elapsed_secs(0) - 1.0 / 60.0).abs() < 1e-6);
    assert!(
        (elapsed_secs(3599) - 60.0).abs() < 1e-3,
        "one simulated minute"
    );
    assert!((elapsed_secs(1799) - 30.0).abs() < 1e-3);
}

/// The trend reading has to separate the case the instrument exists for — a
/// world grinding one way — from a world that is merely alive.
#[test]
fn a_trend_separates_a_one_way_drift_from_a_world_that_breathes() {
    let climbing = trend(&[0.10, 0.14, 0.19, 0.25, 0.33]);
    assert!((climbing.delta - 0.23).abs() < 1e-5);
    assert_eq!(climbing.monotone, 1.0, "every step went the same way");

    // The same endpoints reached with a detour: the delta is identical and the
    // monotone reading is the only thing that distinguishes them.
    let wandering = trend(&[0.10, 0.35, 0.15, 0.05, 0.33]);
    assert!((wandering.delta - climbing.delta).abs() < 1e-5);
    assert_eq!(
        wandering.monotone, 0.5,
        "two of the four steps went the delta's way"
    );

    // A static control: no travel, and no direction to agree with.
    let flat = trend(&[0.42, 0.42, 0.42, 0.42]);
    assert_eq!((flat.delta, flat.monotone), (0.0, 0.0));

    // Falling is drift too — the sign is carried, not absolute.
    let falling = trend(&[0.9, 0.6, 0.4, 0.1]);
    assert!(falling.delta < 0.0);
    assert_eq!(falling.monotone, 1.0);

    // Degenerate series are totals, not panics.
    assert_eq!(trend(&[]).delta, 0.0);
    let single = trend(&[0.5]);
    assert_eq!(
        (single.first, single.last, single.monotone),
        (0.5, 0.5, 0.0)
    );
}

#[test]
fn the_table_prints_a_row_per_interval_and_marks_the_row_with_no_predecessor() {
    let run = a_run(vec![
        sample(0, 0.30, 4.0, None),
        sample(1800, 0.22, 6.0, Some(0.12)),
        sample(3600, 0.14, 9.5, Some(0.11)),
    ]);
    let table = text_table("--presets presets", &run);

    for needle in [
        "Subject",
        "--presets presets",
        "sim_secs",
        "coverage",
        "peak/mean",
        "footprint",
    ] {
        assert!(table.contains(needle), "no `{needle}` in:\n{table}");
    }
    // Three data rows, at their simulated times.
    for secs in ["0.0", "30.0", "60.0"] {
        assert!(
            table.lines().any(|l| l.trim_start().starts_with(secs)),
            "no row at {secs}s:\n{table}"
        );
    }
    // The first row has no predecessor, and must not print a zero there — a
    // zero reads as a frozen world, which is the finding this mode looks for.
    let first = table
        .lines()
        .find(|l| l.trim_start().starts_with("0.0"))
        .expect("the first data row");
    assert!(
        first.trim_end().ends_with('-'),
        "the first row's motion cell is not marked absent: `{first}`"
    );

    // The trend block carries all three statistics and the drift is visible.
    assert!(table.contains("monotone"), "no trend block:\n{table}");
    assert!(
        table.contains("-0.1600"),
        "the coverage delta (0.30 -> 0.14) is not in the trend block:\n{table}"
    );
    // The one thing this instrument must never be read as.
    assert!(
        table.contains("not a gate"),
        "the table does not say it is not a gate:\n{table}"
    );
    // The cost block names the machine rather than asserting anything about it.
    assert!(table.contains(std::env::consts::OS), "no machine:\n{table}");
    assert!(table.contains("3600 frames"), "no frame count:\n{table}");
    assert!(table.contains("400 -> 410 MB"), "no resident set:\n{table}");
    assert!(
        !table.contains("NaN") && !table.contains("inf"),
        "a non-finite value reached the table:\n{table}"
    );
}

#[test]
fn the_json_carries_every_row_and_distinguishes_absent_motion_from_none() {
    let run = a_run(vec![
        sample(0, 0.30, 4.0, None),
        sample(1800, 0.22, 6.0, Some(0.0)),
    ]);
    let json = json_report("embedded defaults", &run);

    for key in [
        "\"source\":",
        "\"preset\":",
        "\"samples\":",
        "\"trend\":",
        "\"cost\":",
        "\"peak_to_mean\":",
        "\"monotone\":",
        "\"software_adapter\":false",
        "\"os\":",
    ] {
        assert!(json.contains(key), "the JSON is missing {key}:\n{json}");
    }

    // The distinction a consumer cannot recover otherwise: the first row has no
    // predecessor (null), the second measured no motion (0.0000).
    assert!(
        json.contains("\"footprint_diff\":null"),
        "the reference row's motion is not null:\n{json}"
    );
    assert!(
        json.contains("\"footprint_diff\":0.0000"),
        "a measured zero must not be null:\n{json}"
    );

    // The footprint series is one shorter than the others, so its trend is
    // taken over the measured rows only rather than over a fabricated zero.
    let motion = trend(&[0.0]);
    assert_eq!(motion.first, 0.0);
    assert!(json.contains("\"footprint\":{\"first\":0.0000"));
}

/// The background is sampled once and held, so a statistic cannot move because
/// the ruler moved.
#[test]
fn the_ground_is_sampled_once_for_the_whole_run() {
    // The figure sits in the **last** pixels, not the first: the top-left pixel
    // is what `corner` samples as the ground, so a fixture that lit it would be
    // measuring its own figure as the background.
    let img = |bg: u8, lit: u32| CaptureImage {
        width: 8,
        height: 8,
        rgba: (0..64u32)
            .flat_map(|i| {
                if i >= 64 - lit {
                    [255, 255, 255, 255]
                } else {
                    [bg, bg, bg, 255]
                }
            })
            .collect(),
    };

    // Two frames whose figure is identical and whose *ground* lifts. Held
    // against the first frame's ground, the second frame's lifted backdrop is
    // itself lit — which is the honest reading, and the point is that it is the
    // same reading `coverage` would give a caller passing that ground by hand.
    let frames = [0u32, 60];
    let samples = measure(&frames, &[img(0, 8), img(120, 8)]);
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].frame, 0);
    assert_eq!(samples[1].frame, 60);
    assert!(
        (samples[0].coverage - 8.0 / 64.0).abs() < 1e-4,
        "8 of 64 pixels lit: {}",
        samples[0].coverage
    );
    assert_eq!(
        samples[1].coverage, 1.0,
        "the lifted ground reads lit against the run's own first-frame ground"
    );
    assert!(samples[0].footprint_diff.is_none(), "no predecessor");
    assert!(samples[1].footprint_diff.is_some());

    // An empty capture is not a panic — the mode reports nothing rather than
    // dividing by an absent frame.
    assert!(measure(&[], &[]).is_empty());
}
