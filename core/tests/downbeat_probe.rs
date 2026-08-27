//! Plan 0068 — the downbeat estimator's terms, made visible.
//!
//! The estimator locks on 3.1 % of audible time (ADR-0082), and the only
//! instrument for that is a 1 Hz column printing the *outcome*. Three terms
//! could be responsible — the accent feature, the 4/4 fold, the confidence
//! measure — and an outcome cannot tell them apart. This file prints the terms.
//!
//! **The deliverable is the printed table, not the pass/fail.** The assertions
//! here are a floor: they pin the properties the print is supposed to
//! illustrate, so a silent regression cannot leave a plausible-looking table
//! behind. To read the table:
//!
//! ```text
//! cargo nextest run -p lmv-core --test downbeat_probe --no-capture
//! cargo test -p lmv-core --test downbeat_probe -- --nocapture
//! ```
//!
//! (`.config/nextest.toml` also keeps these two audible on a passing run, the
//! same override the other two reporting tests carry.)
//!
//! Per ADR-0071, every number printed here is a **measurement** and every
//! number asserted is a **property**: the assertions are comparisons between
//! two readings taken in the same run, so the machine cancels. No absolute
//! confidence value is asserted.

use lmv_core::dsp::downbeat::{BEATS_PER_BAR, BarClock, DownbeatTerms, DownbeatTracker};

/// The gate the estimator publishes above. Not re-exported by the module (it is
/// deliberately private), so it is restated here — if it ever moves, this file's
/// crossings become wrong in an obvious way rather than a subtle one.
const CONFIDENCE_THRESHOLD: f32 = 0.25;

/// Beats the probe drives before reading. Five times the 32-beat history, so the
/// ring is full of the pattern under test and the cold start is long gone.
const BEATS: u32 = 160;

/// A decisive accent and the level of an unaccented beat, as `(bass, onset)`.
/// The same pair the module's own tests use, so the clean rung of every ladder
/// below reproduces a case that is already pinned.
const STRONG: (f32, f32) = (0.9, 0.8);
const WEAK: (f32, f32) = (0.25, 0.2);

/// Drive `beats` beats, one hop each, and return the last published clock.
fn drive(
    tracker: &mut DownbeatTracker,
    beats: u32,
    accent_at: impl Fn(u32) -> (f32, f32),
) -> BarClock {
    let mut last = BarClock::default();
    for index in 0..beats {
        let (bass, onset) = accent_at(index);
        last = tracker.process(true, index, bass, onset, 0.0);
    }
    last
}

/// Header for the decomposition table.
fn header() -> String {
    format!(
        "{:<34} {:>5}  {:>31}  {:>4} {:>4}  {:>6} {:>6} {:>6}  {:>6}",
        "case",
        "beats",
        "fold: mean accent per alignment",
        "best",
        "held",
        "eta^2",
        "null",
        "conf",
        "locked"
    )
}

/// One row of the decomposition table.
fn row(label: &str, t: &DownbeatTerms) -> String {
    let scores = t
        .scores
        .iter()
        .map(|s| format!("{s:.3}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{:<34} {:>5}  {:>31}  {:>4} {:>4}  {:>6.3} {:>6.3} {:>6.3}  {:>6}",
        label,
        t.beats_seen,
        scores,
        t.best,
        t.held,
        t.effect_raw,
        t.null_share,
        t.effect_corrected,
        if t.locked { "yes" } else { "no" }
    )
}

/// Spread of the fold, as a fraction of its own mean — dimensionless, so it
/// compares across cases whose absolute accent levels differ.
fn relative_spread(t: &DownbeatTerms) -> f32 {
    let mean = t.scores.iter().sum::<f32>() / t.scores.len() as f32;
    if mean <= f32::EPSILON {
        return 0.0;
    }
    let max = t.scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min = t.scores.iter().copied().fold(f32::INFINITY, f32::min);
    (max - min) / mean
}

/// A clean 4/4: a decisive accent on beat 1, everything else at the floor.
fn clean(offset: u32) -> impl Fn(u32) -> (f32, f32) {
    move |index| {
        if index % BEATS_PER_BAR == offset {
            STRONG
        } else {
            WEAK
        }
    }
}

/// Phase 1 — the estimator's terms become observable.
///
/// Two cases with known ground truth, printed side by side: a clean 4/4 whose
/// downbeat is at a known offset, and an unaccented click train that has no
/// downbeat to find. The point of printing both is that the *reason* they score
/// as they do becomes visible — the clean case's fold separates by a wide
/// margin and the correction barely dents it; the unaccented case's fold is
/// flat, so there is nothing for the correction to keep.
#[test]
fn the_estimator_decomposes_into_its_terms() {
    println!("\nPlan 0068 Phase 1 — decomposition of the estimator's terms");
    println!("(gate = {CONFIDENCE_THRESHOLD}; conf is what the gate compares)\n");
    println!("{}", header());

    // The clean pattern, in all four rotations — one rotation passing would be
    // consistent with a tracker that always answers 0.
    let mut clean_terms = Vec::new();
    for offset in 0..BEATS_PER_BAR {
        let mut t = DownbeatTracker::new();
        let clock = drive(&mut t, BEATS, clean(offset));
        let terms = t.terms();
        println!(
            "{}",
            row(&format!("clean 4/4, accent on phase {offset}"), &terms)
        );

        // The probe reads the gate, not a second opinion about it: between hops
        // the recomputation must reproduce the published value exactly.
        assert_eq!(
            terms.effect_corrected.to_bits(),
            clock.confidence.to_bits(),
            "offset {offset}: the probe's effect size must be the published confidence, bit for bit"
        );
        assert_eq!(
            terms.locked, clock.locked,
            "offset {offset}: the probe's lock must be the published lock"
        );

        // The true alignment is the largest of the four, and it is the one held.
        assert_eq!(
            terms.best, offset,
            "offset {offset}: the fold should favour the accented alignment, scores {:?}",
            terms.scores
        );
        assert_eq!(
            terms.held, offset,
            "offset {offset}: and should have moved to it"
        );
        assert!(
            terms.locked,
            "offset {offset}: a decisive accent should clear the gate (conf {:.3})",
            terms.effect_corrected
        );
        clean_terms.push(terms);
    }

    // The unaccented click train, flat and noisy. The noisy one is the case
    // that matters: perfectly equal accents make between-group variance exactly
    // zero, which any measure reports as nothing.
    let mut unaccented_terms = Vec::new();
    for (label, jitter) in [("flat", 0.0f32), ("noisy", 0.35)] {
        let mut t = DownbeatTracker::new();
        let clock = drive(&mut t, BEATS, |index| {
            // Deterministic, unstructured, and deliberately NOT periodic in 4.
            let wobble = jitter * (index as f32 * 2.399_963).sin();
            (
                (0.5 + wobble).clamp(0.0, 1.0),
                (0.5 - wobble).clamp(0.0, 1.0),
            )
        });
        let terms = t.terms();
        println!(
            "{}",
            row(&format!("unaccented click train, {label}"), &terms)
        );

        assert_eq!(
            terms.effect_corrected.to_bits(),
            clock.confidence.to_bits(),
            "{label}: the probe's effect size must be the published confidence, bit for bit"
        );
        assert!(
            !terms.locked,
            "{label}: an unaccented train must not lock (conf {:.3})",
            terms.effect_corrected
        );
        unaccented_terms.push(terms);
    }

    println!();

    // The property behind the two rows, stated as a ratio taken in this same
    // run so no absolute number is asserted (ADR-0071). A fold that has found a
    // downbeat is spread wide; a fold with nothing to find is flat.
    let clean_spread = clean_terms
        .iter()
        .map(relative_spread)
        .fold(f32::INFINITY, f32::min);
    let unaccented_spread = unaccented_terms
        .iter()
        .map(relative_spread)
        .fold(f32::NEG_INFINITY, f32::max);
    println!(
        "fold spread (max-min, as a fraction of the fold's mean): \
         clean >= {clean_spread:.3}, unaccented <= {unaccented_spread:.3}, \
         ratio {:.1}x",
        clean_spread / unaccented_spread.max(f32::EPSILON)
    );
    assert!(
        clean_spread > unaccented_spread * 4.0,
        "the accented fold ({clean_spread:.3}) should separate far more than the \
         unaccented one ({unaccented_spread:.3})"
    );

    // And the correction is not what distinguishes them: the raw effect already
    // does. This is the reading that would indict the confidence measure if it
    // ever inverted.
    let clean_raw = clean_terms
        .iter()
        .map(|t| t.effect_raw)
        .fold(f32::INFINITY, f32::min);
    let unaccented_raw = unaccented_terms
        .iter()
        .map(|t| t.effect_raw)
        .fold(f32::NEG_INFINITY, f32::max);
    println!(
        "raw eta^2 before the correction: clean >= {clean_raw:.3}, \
         unaccented <= {unaccented_raw:.3}"
    );
    assert!(
        clean_raw > unaccented_raw,
        "the raw effect size should already separate the two cases"
    );
    println!();
}

// ---------------------------------------------------------------------------
// Phase 2 — the degradation ladder
// ---------------------------------------------------------------------------

/// Rungs per axis: `0.0` (the clean pattern) to `1.0` (fully degraded) in
/// tenths. Every axis is parameterized on the same `0..1` scale and every axis's
/// `1.0` is the end of what that axis can do, so "how far along its own range
/// the estimator survives" is a **dimensionless fraction** and the three are
/// comparable. That comparability is the whole point: the deliverable is which
/// axis is steep, not a crossing number.
const RUNGS: usize = 11;

/// Seeded trials per rung on the two stochastic axes. Every seed is derived
/// from `SEED` below, so the ladder is a pure function of this file — ADR-0071's
/// property form needs the run to be the same run everywhere.
const TRIALS: u64 = 12;

/// Fixed root seed. Arbitrary and written down, which is the requirement.
const SEED: u64 = 0x0068_D0BE_A712_CE55;

/// Readings start here, so every reported number comes from a ring that has
/// been full of the pattern under test for three history-lengths.
const MEASURE_FROM: u32 = 96;

/// Which alignment the synthetic downbeat sits on. Deliberately not `0`: the
/// fallback answers `0`, so an alignment column that reads `2` is evidence the
/// fold found something rather than evidence it did nothing.
const OFFSET: u32 = 2;

/// The largest timing error the jitter axis applies, in beats. One whole beat is
/// the end of the axis because the model below is linear interpolation: at one
/// beat of error the accent has moved entirely onto the neighbour, and there is
/// no further to go.
const MAX_JITTER_BEATS: f32 = 1.0;

/// Beat-to-beat accent variation present on **every** rung of every axis, as a
/// fraction of the accent range — the "no two kicks are the same" floor.
///
/// **This is not a decoration, it is what makes the contrast axis mean
/// anything.** The confidence measure is eta-squared, a between/within variance
/// ratio, and it is therefore *scale-free*: on a perfectly repeatable synthetic
/// the within-group variance is exactly zero, so an accent 1 % louder than its
/// neighbours scores exactly as well as one 100 % louder. The zero-noise control
/// printed below shows that flat line. A real accent feature is compared against
/// its own variability, so the ladder is run against a floor, and the sensitivity
/// table reports how much the answer depends on which floor was assumed.
const NOISE: f32 = 0.10;

/// Floors the sensitivity table sweeps. `0.0` is the degenerate control.
const NOISE_FLOORS: [f32; 4] = [0.0, 0.05, 0.10, 0.25];

/// xorshift64* — three lines, no dependency, and explicitly seeded so nothing
/// here is unseeded randomness (CLAUDE.md's determinism rule).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // The xor keeps a zero seed out of xorshift's fixed point.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `[-1, 1)`.
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// The three ways a real arrangement fails to be the clean pattern, degraded one
/// at a time so a collapse is attributable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// Beat 1 stops being louder than beats 2-4. At `1.0` every beat is
    /// identical — there is no downbeat in the signal to find.
    Contrast,
    /// The detected beat lands early or late, so the accent is measured off its
    /// transient. Zero-mean per beat, which is what "jitter" means and is why
    /// this axis tests the **fold's averaging** rather than the fold's
    /// hypothesis: a persistent phase offset would simply be a different
    /// alignment, correctly found.
    Jitter,
    /// Beats with nothing on them, as a sparse arrangement produces. At `1.0`
    /// every beat is silent.
    Dropout,
}

impl Axis {
    fn label(self) -> &'static str {
        match self {
            Axis::Contrast => "accent contrast lost",
            Axis::Jitter => "timing jitter",
            Axis::Dropout => "dropouts",
        }
    }

    fn units(self) -> &'static str {
        match self {
            Axis::Contrast => "1.0 = beat 1 no louder than beats 2-4",
            Axis::Jitter => "1.0 = timing error uniform over +/- 1 beat",
            Axis::Dropout => "1.0 = every beat silent",
        }
    }
}

/// An accent level between the unaccented floor and the decisive accent.
fn level(w: f32) -> (f32, f32) {
    (
        WEAK.0 + w * (STRONG.0 - WEAK.0),
        WEAK.1 + w * (STRONG.1 - WEAK.1),
    )
}

/// One beat's worth of accent for `axis` at degradation `d`, as the sequence the
/// tracker will be fed. Built up-front rather than per hop because the
/// stochastic axes need state; the tracker itself sees the same shape either
/// way.
fn ladder_pattern(axis: Axis, d: f32, noise: f32, seed: u64, beats: u32) -> Vec<(f32, f32)> {
    let mut rng = Rng::new(seed);
    // A second stream so the noise floor is the same draw at every rung of an
    // axis: only the degradation moves between rungs, which is what "degraded
    // one at a time" requires.
    let mut floor = Rng::new(seed ^ 0xA5A5_5A5A_A5A5_5A5A);
    (0..beats)
        .map(|index| {
            let on_downbeat = index % BEATS_PER_BAR == OFFSET;
            let wobble = noise * floor.signed();
            let level = |w: f32| level((w + wobble).clamp(0.0, 1.0));
            match axis {
                Axis::Contrast => {
                    if on_downbeat {
                        level(1.0 - d)
                    } else {
                        level(0.0)
                    }
                }
                Axis::Jitter => {
                    // The beat is detected at `index + e` instead of `index`, so
                    // the transient's energy splits linearly between the two
                    // beats it now falls between. Linear interpolation conserves
                    // the total accent, which is why this axis is contrast-free:
                    // it moves accent around, it does not remove any.
                    let e = d * MAX_JITTER_BEATS * rng.signed();
                    let pos = index as f32 + e;
                    let mut w = 0.0f32;
                    for delta in -2i64..=2 {
                        let m = index as i64 + delta;
                        if m >= 0 && (m - OFFSET as i64).rem_euclid(BEATS_PER_BAR as i64) == 0 {
                            w += (1.0 - (pos - m as f32).abs()).max(0.0);
                        }
                    }
                    level(w.clamp(0.0, 1.0))
                }
                Axis::Dropout => {
                    if rng.unit() < d {
                        // Nothing played on this beat at all — not a quiet beat,
                        // an empty one.
                        (0.0, 0.0)
                    } else if on_downbeat {
                        level(1.0)
                    } else {
                        level(0.0)
                    }
                }
            }
        })
        .collect()
}

/// What one rung reads, averaged over the reading window and over the trials.
#[derive(Clone, Copy, Default)]
struct Rung {
    d: f32,
    /// Mean raw between-group share, before the noise correction.
    raw: f32,
    /// Mean published confidence — what the gate compares.
    conf: f32,
    /// Fraction of the window in which the gate was open.
    locked: f32,
    /// Fraction of the window in which the fold's `best` was the true
    /// alignment. **This is the column that separates the suspects**: an
    /// alignment that stays right while confidence falls under the gate is the
    /// confidence measure under-reporting, not the fold failing.
    aligned: f32,
}

/// Drive one axis at one degradation level and read the terms over the window.
fn climb(axis: Axis, d: f32, noise: f32) -> Rung {
    let mut acc = Rung {
        d,
        ..Default::default()
    };
    for trial in 0..TRIALS {
        let seed = SEED
            .wrapping_add(trial.wrapping_mul(0x1000_0001))
            .wrapping_add((axis as u64) << 32);
        let pattern = ladder_pattern(axis, d, noise, seed, BEATS);
        let mut t = DownbeatTracker::new();
        let (mut raw, mut conf, mut locked, mut aligned) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut n = 0.0f32;
        for (index, &(bass, onset)) in pattern.iter().enumerate() {
            t.process(true, index as u32, bass, onset, 0.0);
            if index as u32 >= MEASURE_FROM {
                let terms = t.terms();
                raw += terms.effect_raw;
                conf += terms.effect_corrected;
                locked += f32::from(u8::from(terms.locked));
                aligned += f32::from(u8::from(terms.best == OFFSET));
                n += 1.0;
            }
        }
        acc.raw += raw / n;
        acc.conf += conf / n;
        acc.locked += locked / n;
        acc.aligned += aligned / n;
    }
    let trials = TRIALS as f32;
    acc.raw /= trials;
    acc.conf /= trials;
    acc.locked /= trials;
    acc.aligned /= trials;
    acc
}

/// A whole axis at one noise floor.
fn ladder(axis: Axis, noise: f32) -> Vec<Rung> {
    (0..RUNGS)
        .map(|i| climb(axis, i as f32 / (RUNGS - 1) as f32, noise))
        .collect()
}

/// Where `read` falls under the gate, as a fraction of the axis's own range —
/// linearly interpolated between the bracketing rungs. `None` means the axis was
/// traversed end to end without ever falling under the gate.
fn crossing_by(rungs: &[Rung], read: impl Fn(&Rung) -> f32) -> Option<f32> {
    let mut prev: Option<&Rung> = None;
    for r in rungs {
        if read(r) < CONFIDENCE_THRESHOLD {
            return Some(match prev {
                Some(p) if read(p) > read(r) => {
                    let t = (read(p) - CONFIDENCE_THRESHOLD) / (read(p) - read(r));
                    p.d + t * (r.d - p.d)
                }
                _ => r.d,
            });
        }
        prev = Some(r);
    }
    None
}

/// Where the **published** confidence falls under the gate — the real crossing.
fn crossing(rungs: &[Rung]) -> Option<f32> {
    crossing_by(rungs, |r| r.conf)
}

/// Where the crossing *would* be if the noise correction were not applied. The
/// gap between this and [`crossing`] is exactly how much of the shortfall the
/// correction is responsible for, which is one of the plan's three suspects
/// asked directly rather than inferred.
fn crossing_uncorrected(rungs: &[Rung]) -> Option<f32> {
    crossing_by(rungs, |r| r.raw)
}

/// The first rung whose published confidence is under the gate.
fn first_sub_gate(rungs: &[Rung]) -> Option<&Rung> {
    rungs.iter().find(|r| r.conf < CONFIDENCE_THRESHOLD)
}

/// Phase 2 — the degradation ladder: where the lock is actually lost.
///
/// The clean pattern from Phase 1, degraded along three independent axes one at
/// a time, with the confidence read at every rung. The useful output is the
/// **shape** of each curve, not a single crossing number, which is why the whole
/// ladder prints and only the comparison between axes is asserted (ADR-0071: a
/// confidence value at a rung is a measurement of this machine; "contrast is
/// steeper than jitter" is a property of the estimator).
///
/// How the reading separates the three suspects, per the plan:
///
/// - collapse on **contrast loss** indicts the accent feature;
/// - survival of contrast loss but collapse on **dropouts** indicts the fold's
///   history window;
/// - the fold's `best` staying correct while the **published** confidence falls
///   under the gate indicts the noise correction — the `aligned` column is that
///   test, and it is the one that says the estimator knew the answer and
///   declined to publish it.
#[test]
fn the_degradation_ladder_says_which_axis_is_steep() {
    println!("\nPlan 0068 Phase 2 - the degradation ladder");
    println!(
        "(gate = {CONFIDENCE_THRESHOLD}; {BEATS} beats per run, read from beat {MEASURE_FROM}; \
         {TRIALS} seeded trials per rung; beat-to-beat accent noise floor {NOISE})\n"
    );

    let mut ladders = Vec::new();
    for axis in [Axis::Contrast, Axis::Jitter, Axis::Dropout] {
        println!("axis: {}  ({})", axis.label(), axis.units());
        println!(
            "  {:>6}  {:>8}  {:>8}  {:>8}  {:>10}",
            "rung", "eta^2", "conf", "locked", "aligned"
        );
        let rungs = ladder(axis, NOISE);
        for r in &rungs {
            println!(
                "  {:>6.2}  {:>8.3}  {:>8.3}  {:>7.0}%  {:>9.0}%",
                r.d,
                r.raw,
                r.conf,
                r.locked * 100.0,
                r.aligned * 100.0
            );
        }
        match crossing(&rungs) {
            Some(x) => println!(
                "  -> confidence crosses the gate at {:.0}% of this axis\n",
                x * 100.0
            ),
            None => println!("  -> confidence never falls under the gate on this axis\n"),
        }
        ladders.push((axis, rungs));
    }

    // Summary, and the comparative claim the phase exists to make. The last two
    // columns are the discriminators: `uncorrected` asks how much of the
    // shortfall the noise correction owns, and `aligned` asks whether the fold
    // still knew the answer at the rung where the gate shut on it.
    let cell = |x: Option<f32>| match x {
        Some(x) => format!("{:.0}% of axis", x * 100.0),
        None => "never".to_string(),
    };
    println!(
        "  {:<22} {:>14} {:>16}  {:>26}",
        "axis", "crossing", "uncorrected", "alignment at that rung"
    );
    for (axis, rungs) in &ladders {
        println!(
            "  {:<22} {:>14} {:>16}  {:>26}",
            axis.label(),
            cell(crossing(rungs)),
            cell(crossing_uncorrected(rungs)),
            match first_sub_gate(rungs) {
                Some(r) => format!("{:.0}% correct (rung {:.2})", r.aligned * 100.0, r.d),
                None => "-".to_string(),
            }
        );
    }
    println!();

    // How much of the above is the assumed noise floor talking? The floor is the
    // one number in this harness that is not read off the estimator, so its
    // influence is reported rather than argued about. The `0.00` row is the
    // degenerate control: with perfectly repeatable accents eta-squared is
    // scale-free, so the contrast axis is flat at ceiling until contrast is
    // *exactly* zero — which is the single most useful thing on this page about
    // what the confidence measure actually measures.
    println!("sensitivity of the crossing to the assumed beat-to-beat noise floor");
    println!(
        "  {:>6}  {:>18}  {:>18}  {:>18}",
        "floor",
        Axis::Contrast.label(),
        Axis::Jitter.label(),
        Axis::Dropout.label()
    );
    let mut sensitivity = Vec::new();
    for &noise in NOISE_FLOORS.iter() {
        let row: Vec<(Axis, Option<f32>)> = [Axis::Contrast, Axis::Jitter, Axis::Dropout]
            .into_iter()
            .map(|axis| (axis, crossing(&ladder(axis, noise))))
            .collect();
        println!(
            "  {:>6.2}  {:>18}  {:>18}  {:>18}",
            noise,
            cell(row.first().map(|r| r.1).unwrap_or(None)),
            cell(row.get(1).map(|r| r.1).unwrap_or(None)),
            cell(row.get(2).map(|r| r.1).unwrap_or(None))
        );
        sensitivity.push((noise, row));
    }
    println!();

    // --- properties, not measurements -------------------------------------
    //
    // (1) Rung 0 of every axis is the clean pattern, and it locks. If this ever
    //     fails the ladder is measuring the harness, not the estimator.
    for (axis, rungs) in &ladders {
        let clean = rungs.first().expect("RUNGS > 0");
        assert!(
            clean.locked > 0.99 && clean.aligned > 0.99,
            "{}: rung 0 is the clean pattern and must lock on the true alignment \
             (locked {:.0}%, aligned {:.0}%, conf {:.3})",
            axis.label(),
            clean.locked * 100.0,
            clean.aligned * 100.0,
            clean.conf
        );
    }

    // (2) Every axis degrades: the top rung reads lower than the clean rung.
    //     A ratio inside one run, so the machine cancels.
    for (axis, rungs) in &ladders {
        let clean = rungs.first().expect("RUNGS > 0").conf;
        let worst = rungs.last().expect("RUNGS > 0").conf;
        assert!(
            worst < clean,
            "{}: the top of the axis ({worst:.3}) should read below the clean rung ({clean:.3})",
            axis.label()
        );
    }

    // (3) **Dropouts are the steep axis** — comparative, dimensionless, and it
    //     holds at every noise floor swept above, so it is not an artifact of
    //     the one assumption this harness makes. The 0.75 factor is slack: the
    //     observed ratios sit near 0.5.
    for (noise, row) in &sensitivity {
        let get = |want: Axis| {
            row.iter()
                .find(|(axis, _)| *axis == want)
                .map(|(_, x)| *x)
                .expect("every axis is on the row")
        };
        let dropout = get(Axis::Dropout)
            .unwrap_or_else(|| panic!("floor {noise}: the dropout axis must cross the gate"));
        let contrast = get(Axis::Contrast)
            .unwrap_or_else(|| panic!("floor {noise}: the contrast axis must cross the gate"));
        assert!(
            dropout < contrast * 0.75,
            "floor {noise}: dropouts should be far steeper than contrast loss \
             (dropout {dropout:.2}, contrast {contrast:.2} of their axes)"
        );
        // Jitter is the shallowest of the three: it either never crosses, or it
        // crosses later than dropouts do.
        match get(Axis::Jitter) {
            None => {}
            Some(j) => assert!(
                j > dropout,
                "floor {noise}: timing jitter ({j:.2}) should tolerate more of its axis \
                 than dropouts ({dropout:.2}) do"
            ),
        }
    }

    // (4) **The fold still knows the answer at the rung where the gate shuts on
    //     it.** This is the discriminator the plan asks for: an alignment that
    //     is right while the confidence is under the gate says the fold found
    //     the downbeat and the confidence measure declined to publish it. Both
    //     numbers come from the same rung of the same run.
    let dropouts = ladders
        .iter()
        .find(|(axis, _)| *axis == Axis::Dropout)
        .map(|(_, rungs)| rungs)
        .expect("the dropout axis is on the ladder");
    let shut = first_sub_gate(dropouts).expect("the dropout axis crosses the gate");
    println!(
        "verdict input: at dropout rung {:.2} the gate is open {:.0}% of the window \
         while the fold names the true alignment {:.0}% of it",
        shut.d,
        shut.locked * 100.0,
        shut.aligned * 100.0
    );
    assert!(
        shut.aligned > 0.9 && shut.locked < 0.5,
        "at the first sub-gate dropout rung the fold should still be right far more often \
         than the gate is open (aligned {:.0}%, locked {:.0}%)",
        shut.aligned * 100.0,
        shut.locked * 100.0
    );
    println!();
}
