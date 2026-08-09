//! Plan 0068 — the downbeat estimator's terms, made visible.
//!
//! The estimator locks on 3.1 % of audible time ([ADR-0082]), and the only
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
//! Per [ADR-0071], every number printed here is a **measurement** and every
//! number asserted is a **property**: the assertions are comparisons between
//! two readings taken in the same run, so the machine cancels. No absolute
//! confidence value is asserted.
//!
//! [ADR-0082]: ../../docs/adrs/0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md
//! [ADR-0071]: ../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md

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
