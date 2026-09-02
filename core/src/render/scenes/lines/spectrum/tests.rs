// Test asserts index the produced Vec; allowed here over the file's
// hot-path pragma since test code is not the render path.
#![allow(clippy::indexing_slicing)]

use super::*;
use crate::dsp::SPECTRUM_BINS;

/// The cap these tests run at — the floor tier's, which is the value they were
/// written against and the one every shipped preset is authored and gated on
/// (Plan 0044).
const FLOOR_CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

fn white(n: usize) -> Vec<[f32; 3]> {
    vec![[1.0, 1.0, 1.0]; n]
}

/// The per-element half-width [`even_widths`] hands every element, and so the
/// extension a joined end carries in these tests.
const W: f32 = 0.01;

fn even_widths(n: usize) -> Vec<f32> {
    vec![W; n]
}

/// Build one figure from raw per-element lengths at an explicit placement.
fn placed(layout: SpectrumLayout, lengths: &[f32], place: Placement) -> Vec<SegmentInstance> {
    let mut out = Vec::new();
    build(
        layout,
        lengths,
        &even_widths(lengths.len()),
        &white(lengths.len()),
        place,
        &mut out,
    );
    out
}

/// Build one figure from raw per-element lengths, at the default placement,
/// unturned.
fn figure(layout: SpectrumLayout, lengths: &[f32]) -> Vec<SegmentInstance> {
    placed(layout, lengths, Placement::default())
}

/// The element lengths a stimulus produces, low index first — the whole
/// chain from band array to drawn geometry, at `base = 0`, `scale = 1`.
fn bar_lengths(spectrum: &[f32; SPECTRUM_BINS], elements: usize) -> Vec<f32> {
    let mut levels = vec![0.0; elements];
    downsample(spectrum, &mut levels);
    let lengths: Vec<f32> = levels
        .iter()
        .map(|&level| element_length(level, 0.0, 1.0))
        .collect();
    figure(SpectrumLayout::Bars, &lengths)
        .iter()
        .map(|s| s.b[1] - s.a[1])
        .collect()
}

/// A band array with energy `1.0` in `lit` bands starting at `from`.
fn banded(from: usize, lit: usize) -> [f32; SPECTRUM_BINS] {
    let mut spectrum = [0.0; SPECTRUM_BINS];
    for band in spectrum.iter_mut().skip(from).take(lit) {
        *band = 1.0;
    }
    spectrum
}

/// Plan 0034 Phase 2 done-when 3, and the claim that separates a working
/// spectrum from N bars of noise: **energy lands on the elements that own
/// its frequencies.** Bass raises the low-index bars and leaves the high
/// ones flat; treble does exactly the reverse.
#[test]
fn energy_raises_the_elements_that_own_its_frequencies() {
    let n = 24;
    let quarter = n / 4;

    let low = bar_lengths(&banded(0, SPECTRUM_BINS / 4), n);
    let high = bar_lengths(&banded(3 * SPECTRUM_BINS / 4, SPECTRUM_BINS / 4), n);

    for i in 0..quarter {
        assert!(
            low[i] > 0.5,
            "bass stimulus must raise low element {i}, got {}",
            low[i]
        );
        assert_eq!(
            high[i], 0.0,
            "treble stimulus must leave low element {i} flat"
        );
    }
    for i in (n - quarter)..n {
        assert!(
            high[i] > 0.5,
            "treble stimulus must raise high element {i}, got {}",
            high[i]
        );
        assert_eq!(
            low[i], 0.0,
            "bass stimulus must leave high element {i} flat"
        );
    }
}

/// Plan 0034 Phase 3 done-when 2. The downsample is a genuine **partition**
/// of the band array at every element count the loader admits: contiguous,
/// non-overlapping, complete. Proved by conservation — the element means,
/// weighted by how many bands each covers, must sum to the band total, which
/// is only possible if every band was counted exactly once.
#[test]
fn every_band_is_counted_exactly_once_at_every_element_count() {
    // A distinct value per band, so a dropped or double-counted band shifts
    // the total rather than cancelling out.
    let mut spectrum = [0.0f32; SPECTRUM_BINS];
    for (i, band) in spectrum.iter_mut().enumerate() {
        *band = (i + 1) as f32;
    }
    let total: f32 = spectrum.iter().sum();

    // Every count the loader admits, walked end to end at the small sizes
    // and sampled at the awkward ones (7, 24, 30, 31 do not divide 64).
    for n in 2..=SPECTRUM_BINS {
        let mut levels = vec![0.0; n];
        downsample(&spectrum, &mut levels);

        let mut covered = 0usize;
        let mut weighted = 0.0f32;
        for (i, &level) in levels.iter().enumerate() {
            let lo = i * SPECTRUM_BINS / n;
            let hi = (i + 1) * SPECTRUM_BINS / n;
            assert_eq!(
                lo,
                covered,
                "element {i} of {n} must start where element {} ended",
                i.saturating_sub(1)
            );
            assert!(hi > lo, "element {i} of {n} must cover at least one band");
            covered = hi;
            weighted += level * (hi - lo) as f32;
        }
        assert_eq!(covered, SPECTRUM_BINS, "{n} elements must cover every band");
        assert!(
            (weighted - total).abs() < 1e-2,
            "{n} elements lose or duplicate energy: {weighted} vs {total}"
        );
    }
}

/// The bars layout is what a spectrum readout should be: one segment per
/// element, evenly spaced left to right, standing on a common baseline.
#[test]
fn bars_are_evenly_spaced_upright_segments_on_one_baseline() {
    let levels = [0.0f32, 0.5, 1.0, 0.25];
    let plain: Vec<f32> = levels
        .iter()
        .map(|&level| element_length(level, 0.0, 1.0))
        .collect();
    let out = figure(SpectrumLayout::Bars, &plain);
    assert_eq!(out.len(), levels.len(), "one segment per element");

    let spacing = out[1].a[0] - out[0].a[0];
    for (i, seg) in out.iter().enumerate() {
        assert_eq!(seg.a[0], seg.b[0], "element {i} is upright");
        assert_eq!(
            seg.a[1], DEFAULT_BASELINE,
            "element {i} stands on the baseline"
        );
        assert!(
            seg.a[0].abs() <= DEFAULT_SPAN,
            "element {i} stays inside the span"
        );
        if i > 0 {
            assert!(
                ((out[i].a[0] - out[i - 1].a[0]) - spacing).abs() < 1e-6,
                "elements are evenly spaced"
            );
        }
    }
    // Length tracks the level, and `base` lifts every element off the floor.
    // Compared approximately: a length is a difference of two world-space
    // f32 coordinates, so it carries the baseline's rounding.
    let length = |seg: &SegmentInstance| seg.b[1] - seg.a[1];
    assert!(length(&out[0]).abs() < 1e-6, "a silent element is flat");
    assert!(
        (length(&out[2]) - 1.0).abs() < 1e-6,
        "a full element is full"
    );

    let lifted_lengths: Vec<f32> = levels
        .iter()
        .map(|&level| element_length(level, 0.1, 1.0))
        .collect();
    let lifted = figure(SpectrumLayout::Bars, &lifted_lengths);
    assert!(
        (length(&lifted[0]) - 0.1).abs() < 1e-6,
        "base is the resting length, so silence still draws a comb"
    );
    // And a level can never pull an element below its baseline, whatever a
    // degenerate `scale` does.
    assert_eq!(element_length(1.0, 0.0, -5.0), 0.0, "length floors at zero");
}

/// Plan 0038 Phase 3 done-when 2, ADR-0040's totality clause. `curve_level`
/// runs per element per frame on the render path with no author guard in
/// front of it, so **every** degenerate combination has to land on a finite,
/// non-negative number — not merely the ones an author is likely to write.
#[test]
fn the_level_curve_is_total_over_degenerate_input() {
    // `1.0` is exactly the identity, which is the property the whole
    // byte-identical-goldens claim rests on. Asserted bit-for-bit.
    for level in [0.0f32, 0.003, 0.03, 0.5, 1.0, 7.5] {
        assert_eq!(
            curve_level(level, 1.0),
            level,
            "curve = 1.0 must be exactly `powf(x, 1.0) == x` for {level}"
        );
    }

    // The exponent is the author-reachable half: `curve` is an expression, so
    // every one of these is something a preset can actually produce.
    let exponents = [
        0.0f32,
        -0.0,
        -1.0,
        -1e30,
        1e30,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        0.5,
        CURVE_MIN,
        CURVE_MAX,
    ];
    // The level is the engine-side half — it comes from `downsample` over the
    // band array, never from an expression. These span far past anything the
    // DSP produces (bands sit around 0.02-0.05).
    let levels = [0.0f32, -0.0, -1.0, -1e3, f32::MIN_POSITIVE, 0.03, 1.0, 1e3];

    for &curve in &exponents {
        // Whatever the exponent, a level the DSP can produce stays drawable
        // all the way through to the geometry.
        for &level in &levels {
            let out = curve_level(level, curve);
            assert!(
                out.is_finite() && out >= 0.0,
                "curve_level({level}, {curve}) = {out} is not a drawable level"
            );
            let len = element_length(out, 0.06, 1.1);
            assert!(
                len.is_finite() && len >= 0.0,
                "element_length after curve_level({level}, {curve}) = {len}"
            );
        }
        // And the weaker claim that holds for *any* level at all, including
        // the non-finite ones no band array should ever contain: the curve
        // never manufactures a `NaN` and never inverts an element. An
        // infinite level still maps to an infinite length, exactly as it did
        // through `element_length` before this step existed — that is the
        // DSP's boundary to hold, not this function's.
        for &level in &[f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let out = curve_level(level, curve);
            assert!(
                !out.is_nan() && out >= 0.0,
                "curve_level({level}, {curve}) = {out} introduced a NaN or a \
                 negative where the input had neither"
            );
        }
    }
    // A non-finite level that is not `+inf` is neutralised outright, because
    // the floor runs before the `powf`.
    assert_eq!(curve_level(f32::NAN, 0.5), 0.0);
    assert_eq!(curve_level(f32::NEG_INFINITY, 0.5), 0.0);

    // The two clauses that make `pow(0, 0)` and `pow(0, -1)` unreachable: the
    // exponent floor is strictly positive, so a silent element stays at zero
    // rather than jumping to one or to infinity.
    assert_eq!(curve_level(0.0, 0.0), 0.0, "pow(0, 0) is not reachable");
    assert_eq!(curve_level(0.0, -1.0), 0.0, "pow(0, -1) is not reachable");
    assert_eq!(
        curve_level(0.0, f32::NAN),
        0.0,
        "a NaN exponent falls back to the linear default, not through clamp"
    );
    // A negative level can never become a fractional power of a negative
    // base, because it is floored before the `powf` rather than after.
    assert_eq!(curve_level(-4.0, 0.5), 0.0, "a negative level floors first");
    // The exponent clamp bites at both ends rather than passing through.
    assert_eq!(curve_level(0.25, 10.0), curve_level(0.25, CURVE_MAX));
    assert_eq!(curve_level(0.25, 0.0001), curve_level(0.25, CURVE_MIN));
}

/// Plan 0038 Phase 3 done-when 3, and the reason ADR-0040 exists. The
/// ordering is invisible in a still frame — it only shows in *motion* — so
/// nothing but a test stops a later refactor from swapping two lines and
/// silently inverting the decision.
///
/// Asserted as a **property**, not against a tuned constant: for a
/// compressive `curve` and a non-instant smoother, curving-then-easing and
/// easing-then-curving disagree, and they disagree in a *stated direction* —
/// during a fall the curve-first value sits **below** the ease-first one,
/// because curving after the smoother stretches the decay by `1 / curve` and
/// a slower decay is a higher value at every instant. **Swap the two steps in
/// `update` and this fails.**
///
/// The direction is all this asserts. It is *not* evidence for the "even
/// fall" ADR-0040 originally claimed — Plan 0038 Phase 3 measured that and
/// found both orderings to be exponentials of identical shape, differing only
/// in speed (see the ADR's Outcome). The ordering is still worth pinning,
/// because it is invisible in a still frame and a refactor can swap two lines.
#[test]
fn the_curve_runs_before_the_easing_and_the_two_orders_differ() {
    const CURVE: f32 = 0.5;
    const DT: f32 = 1.0 / 60.0;
    let easing = Easing {
        attack: 0.02,
        release: 0.5,
    };

    // A step down from a settled loud element to silence — the fall ADR-0040
    // reasons about. Both orders start from the same settled state, so the
    // only thing that differs downstream is where the curve sits.
    let settled = curve_level(1.0, CURVE);
    let (mut curve_first, mut ease_first) = (settled, 1.0f32);
    let mut saw_difference = false;

    for frame in 0..90 {
        // Curve then ease: the smoother sees the compressed value. This is
        // exactly what `update` does.
        curve_first = easing.step(curve_first, curve_level(0.0, CURVE), DT);
        // Ease then curve: the smoother sees the linear value and the curve
        // is applied to the eased result. The rejected alternative.
        ease_first = easing.step(ease_first, 0.0, DT);
        let ease_first_shown = curve_level(ease_first, CURVE);

        if (curve_first - ease_first_shown).abs() > 1e-6 {
            saw_difference = true;
            assert!(
                curve_first < ease_first_shown,
                "frame {frame}: curve-then-ease ({curve_first}) should sit \
                 below ease-then-curve ({ease_first_shown}) during a fall — \
                 applying the curve after the smoother stretches the decay \
                 by 1 / curve, and the slower fall is the higher value"
            );
        }
    }
    assert!(
        saw_difference,
        "the two orders never diverged, so this test would pass with the \
         steps swapped — the fixture is not exercising ADR-0040 at all"
    );

    // ...and the ordering is a no-op at the default, which is the other half
    // of the claim: `curve = 1.0` makes the two orders identical, so the
    // decision costs nothing until a preset opts in.
    let (mut a, mut b) = (1.0f32, 1.0f32);
    for _ in 0..30 {
        a = easing.step(a, curve_level(0.0, 1.0), DT);
        b = curve_level(easing.step(b, 0.0, DT), 1.0);
        assert_eq!(a, b, "at curve = 1.0 the two orders must coincide");
    }
}

/// Plan 0038 Phase 2. `span` and `baseline` place the figure in **world**
/// space — no aspect and no target size is read anywhere in this scene to
/// compute them (ADR-0037), which is why doubling `span` is exactly doubling
/// an x coordinate and nothing else.
#[test]
fn span_and_baseline_place_the_figure_in_world_space() {
    let lengths = [0.2f32, 0.5, 0.9, 0.4];

    for layout in [SpectrumLayout::Bars, SpectrumLayout::Polyline] {
        let narrow = placed(layout, &lengths, Placement::default());
        let wide = placed(
            layout,
            &lengths,
            Placement {
                span: 2.0 * DEFAULT_SPAN,
                ..Placement::default()
            },
        );
        for (i, (n, w)) in narrow.iter().zip(&wide).enumerate() {
            assert!(
                (w.a[0] - 2.0 * n.a[0]).abs() < 1e-6 && (w.b[0] - 2.0 * n.b[0]).abs() < 1e-6,
                "{layout:?} segment {i}: doubling span must double x, got {} from {}",
                w.a[0],
                n.a[0]
            );
            assert_eq!(
                (w.a[1], w.b[1]),
                (n.a[1], n.b[1]),
                "{layout:?} segment {i}: span must not move y"
            );
        }

        // `baseline` is a pure y offset, again with no x effect.
        let lifted = placed(
            layout,
            &lengths,
            Placement {
                baseline: DEFAULT_BASELINE + 0.5,
                ..Placement::default()
            },
        );
        for (i, (n, l)) in narrow.iter().zip(&lifted).enumerate() {
            assert!(
                (l.a[1] - n.a[1] - 0.5).abs() < 1e-6,
                "{layout:?} segment {i}: baseline must offset y by exactly its change"
            );
            assert_eq!(
                l.a[0], n.a[0],
                "{layout:?} segment {i}: baseline moves no x"
            );
        }
    }

    // Both are no-ops on the ring, which `radius` sizes instead.
    let ring = placed(SpectrumLayout::RadialRing, &lengths, Placement::default());
    let ring_moved = placed(
        SpectrumLayout::RadialRing,
        &lengths,
        Placement {
            span: 4.0,
            baseline: 0.7,
            ..Placement::default()
        },
    );
    assert_eq!(
        ring.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>(),
        ring_moved.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>(),
        "span and baseline are no-ops on the radial ring"
    );
}

/// Plan 0038 Phase 2 done-when 3, the design-backlog 0018 fix. The geometry
/// mirror reflects across the **x-axis** (`lines/mod.rs`) on every line
/// scene alike; nothing about that changes here. What changes is where the
/// figure stands. At `baseline = 0` the two copies share one foot line — the
/// symmetric "landscape and its reflection". At the default `-0.85` they
/// have two feet lines 1.7 apart, so the copy hangs from the top edge
/// instead of mirroring about a shared centre.
#[test]
fn baseline_zero_mirrors_the_readout_about_the_frame_centre() {
    let lengths = [0.2f32, 0.5, 0.9, 0.4];
    let mirror = MirrorSpec::from_params(1.0, 1.0);
    assert!(!mirror.is_identity(), "the probe must actually replicate");

    // The distinct y values the bars' feet (their `a` endpoints) rest on,
    // rounded so float noise cannot invent a second line.
    let feet = |place: Placement| -> Vec<i32> {
        let single = placed(SpectrumLayout::Bars, &lengths, place);
        let mut out = Vec::new();
        replicate_mirror(&single, mirror, FLOOR_CAP, &mut out);
        let mut ys: Vec<i32> = out.iter().map(|s| (s.a[1] * 1e4) as i32).collect();
        ys.sort_unstable();
        ys.dedup();
        ys
    };

    assert_eq!(
        feet(Placement {
            baseline: 0.0,
            ..Placement::default()
        }),
        vec![0],
        "baseline = 0: both copies stand on the one centre line"
    );
    let default_feet = feet(Placement::default());
    assert_eq!(
        default_feet.len(),
        2,
        "the default baseline gives the pair two separate foot lines"
    );
    assert_eq!(
        default_feet,
        vec![-8500, 8500],
        "and they sit at -0.85 and +0.85, 1.7 apart — the copy against the \
         top edge that design-backlog 0018 reported"
    );
}

/// Plan 0034 Phase 3 done-when 1: each layout draws the *same data* as a
/// different figure, and each is structurally what its name claims.
#[test]
fn each_layout_draws_the_same_levels_as_its_own_figure() {
    let levels = [0.1f32, 0.4, 0.9, 0.3, 0.6];

    // Polyline: a connected chain, so consecutive segments share an endpoint
    // and there is one fewer segment than elements.
    let out = figure(SpectrumLayout::Polyline, &levels);
    assert_eq!(out.len(), levels.len() - 1, "n-1 segments join n points");
    for i in 1..out.len() {
        assert_eq!(
            out[i - 1].b,
            out[i].a,
            "segment {i} continues from the previous one"
        );
    }
    // The chain spans the full width, edge to edge.
    assert!(
        (out[0].a[0] + DEFAULT_SPAN).abs() < 1e-6,
        "starts at the left edge"
    );
    assert!(
        (out[out.len() - 1].b[0] - DEFAULT_SPAN).abs() < 1e-6,
        "ends at the right edge"
    );

    // Radial ring: one spoke per element, each pointing away from the origin
    // and starting on a circle of the configured inner radius.
    let mut out = Vec::new();
    build(
        SpectrumLayout::RadialRing,
        &levels,
        &even_widths(levels.len()),
        &white(levels.len()),
        Placement {
            radius: 0.4,
            ..Placement::default()
        },
        &mut out,
    );
    assert_eq!(out.len(), levels.len(), "one spoke per element");
    for (i, seg) in out.iter().enumerate() {
        let inner = (seg.a[0] * seg.a[0] + seg.a[1] * seg.a[1]).sqrt();
        let outer = (seg.b[0] * seg.b[0] + seg.b[1] * seg.b[1]).sqrt();
        assert!(
            (inner - 0.4).abs() < 1e-5,
            "spoke {i} starts on the ring, got {inner}"
        );
        assert!(
            (outer - (0.4 + levels[i])).abs() < 1e-5,
            "spoke {i} reaches its own level outward"
        );
    }
    // The spokes are distinct directions covering the circle once.
    let angle = |seg: &SegmentInstance| seg.a[1].atan2(seg.a[0]);
    let expected = std::f32::consts::TAU / levels.len() as f32;
    for i in 1..out.len() {
        // Wrapped, because `atan2` returns -pi..pi and the walk crosses it.
        let step = (angle(&out[i]) - angle(&out[i - 1])).rem_euclid(std::f32::consts::TAU);
        assert!(
            (step - expected).abs() < 1e-5,
            "spoke {i} sits one even step around the circle, got {step}"
        );
    }
}

/// Plan 0039 Phase 2 done-when 1 and 2 (ADR-0041). This scene is the one
/// producer emitting **both** connectivities from a single `build()`, so it
/// is what proves the flag is per endpoint rather than per scene.
///
/// The isolated half is the load-bearing one: it is the done-when that would
/// have caught the rejected unconditional-extend design. A bar whose `a` end
/// extended would hang below `baseline` — breaking the `baseline = 0`
/// centre-mirror Plan 0038 shipped — and a spoke whose `a` end extended would
/// grow inward through `radius` and fill the inner circle.
#[test]
fn only_the_polyline_extends_its_endpoints() {
    use crate::render::scenes::lines::{MITER_SLACK, expected_miter};

    let levels = [0.1f32, 0.4, 0.9, 0.3, 0.6];

    // Chained: every interior vertex is a joint; the figure's two outer ends
    // stay free, or the stroke would run half a width past each edge.
    let chain = figure(SpectrumLayout::Polyline, &levels);
    assert_eq!(
        chain
            .iter()
            .map(|s| (s.ext_a > 0.0, s.ext_b > 0.0))
            .collect::<Vec<_>>(),
        vec![(false, true), (true, true), (true, true), (true, false)],
        "the interior endpoints of the chain are extended and only those"
    );
    // ...and every extension matches a genuinely shared point AND the miter
    // that point's own interior angle asks for — which is the invariant nothing
    // else in the pipeline validates.
    for i in 1..chain.len() {
        assert_eq!(chain[i - 1].b, chain[i].a, "segment {i} shares a point");
        let want = expected_miter(W, chain[i - 1].a, chain[i].a, chain[i].b);
        assert!(
            (chain[i - 1].ext_b - want).abs() <= want * MITER_SLACK
                && (chain[i].ext_a - want).abs() <= want * MITER_SLACK,
            "vertex {i}: the two sides carry {} and {} against the miter {want} \
             its interior angle asks for",
            chain[i - 1].ext_b,
            chain[i].ext_a
        );
    }
    // Non-vacuity: these five levels put real corners in the readout, so at
    // least one joint reaches well past the flat half-width a bevel would.
    assert!(
        chain.iter().any(|s| s.ext_a > 1.2 * W),
        "no joint in this fixture reaches past {}, so it cannot separate a \
         mitred corner from a bevelled one: {:?}",
        1.2 * W,
        chain.iter().map(|s| s.ext_a).collect::<Vec<_>>()
    );
    // Two elements make one segment, which is all end and no joint.
    let lone = figure(SpectrumLayout::Polyline, &[0.3, 0.7]);
    assert_eq!(lone.len(), 1);
    assert_eq!(
        (lone[0].ext_a, lone[0].ext_b),
        (0.0, 0.0),
        "a lone segment has two free ends"
    );

    // Isolated: one segment per element, both ends free, and the endpoints
    // stay exactly where they always were.
    let bars = figure(SpectrumLayout::Bars, &levels);
    for (i, seg) in bars.iter().enumerate() {
        assert_eq!((seg.ext_a, seg.ext_b), (0.0, 0.0), "bar {i} is isolated");
        assert_eq!(
            seg.a[1], DEFAULT_BASELINE,
            "bar {i} still stands on the baseline"
        );
        assert!(
            (seg.b[1] - (DEFAULT_BASELINE + levels[i])).abs() < 1e-6,
            "bar {i} still ends exactly at baseline + length"
        );
    }

    let ring = placed(
        SpectrumLayout::RadialRing,
        &levels,
        Placement {
            radius: 0.4,
            ..Placement::default()
        },
    );
    for (i, seg) in ring.iter().enumerate() {
        assert_eq!((seg.ext_a, seg.ext_b), (0.0, 0.0), "spoke {i} is isolated");
        let inner = (seg.a[0] * seg.a[0] + seg.a[1] * seg.a[1]).sqrt();
        assert!(
            (inner - 0.4).abs() < 1e-5,
            "spoke {i} still starts exactly on the ring, got {inner}"
        );
    }
}

/// `rotation` turns the whole figure about the origin — the same angle for
/// every layout, so it is never a silent no-op on one of them.
#[test]
fn rotation_turns_every_layout_by_the_same_angle() {
    let levels = [0.2f32, 0.7, 0.4, 0.5];
    let quarter = std::f32::consts::FRAC_PI_2;

    for layout in [
        SpectrumLayout::Bars,
        SpectrumLayout::Polyline,
        SpectrumLayout::RadialRing,
    ] {
        let plain = figure(layout, &levels);
        let mut turned = Vec::new();
        build(
            layout,
            &levels,
            &even_widths(levels.len()),
            &white(levels.len()),
            Placement {
                rotation: quarter,
                ..Placement::default()
            },
            &mut turned,
        );
        assert_eq!(plain.len(), turned.len(), "{layout:?} keeps its segments");
        for (a, b) in plain.iter().zip(&turned) {
            // A quarter turn maps (x, y) to (-y, x).
            assert!(
                (b.a[0] + a.a[1]).abs() < 1e-5 && (b.a[1] - a.a[0]).abs() < 1e-5,
                "{layout:?} start point is not rotated a quarter turn"
            );
            assert!(
                (b.b[0] + a.b[1]).abs() < 1e-5 && (b.b[1] - a.b[0]).abs() < 1e-5,
                "{layout:?} end point is not rotated a quarter turn"
            );
        }
    }
}

/// Plan 0034 Phase 3 done-when 3. Per-element easing is expressed in seconds
/// and is **frame-rate independent**: reaching the same wall-clock time
/// through many small steps or a few large ones lands in the same place, so a
/// preset looks the same at 60 and 144 Hz (ADR-0019).
#[test]
fn per_element_easing_is_frame_rate_independent() {
    let easing = Easing::symmetric(0.25);
    let settle = |steps: u32, dt: f32| {
        let mut held = 0.0f32;
        for _ in 0..steps {
            held = easing.step(held, 1.0, dt);
        }
        held
    };
    // One second of wall clock at 60, 144 and 30 fps.
    let at_60 = settle(60, 1.0 / 60.0);
    let at_144 = settle(144, 1.0 / 144.0);
    let at_30 = settle(30, 1.0 / 30.0);
    assert!(
        (at_60 - at_144).abs() < 1e-3 && (at_60 - at_30).abs() < 1e-3,
        "one second of easing must land in the same place: 60 {at_60}, 144 {at_144}, 30 {at_30}"
    );
    // And it is genuinely easing rather than snapping or stalling.
    assert!(
        at_60 > 0.9 && at_60 < 1.0,
        "a 0.25 s constant is most of the way there after 1 s, got {at_60}"
    );
    // `INSTANT` (the default) passes the raw value straight through.
    assert_eq!(Easing::INSTANT.step(0.0, 0.6, 1.0 / 60.0), 0.6);
}

/// A degenerate element count must not panic or produce garbage geometry —
/// this runs per frame.
#[test]
fn a_degenerate_element_count_is_inert() {
    let mut none: Vec<f32> = Vec::new();
    downsample(&[0.5; SPECTRUM_BINS], &mut none);
    for layout in [
        SpectrumLayout::Bars,
        SpectrumLayout::Polyline,
        SpectrumLayout::RadialRing,
    ] {
        let mut out = vec![SegmentInstance {
            a: [9.0, 9.0],
            b: [9.0, 9.0],
            color: [1.0, 1.0, 1.0],
            width: 1.0,
            alpha: 1.0,
            ext_a: 0.0,
            ext_b: 0.0,
        }];
        build(layout, &none, &[], &[], Placement::default(), &mut out);
        assert!(out.is_empty(), "{layout:?}: no elements, no segments");

        // Short width/colour lists fall back rather than panicking — the hot
        // path must stay total even if the buffers ever disagree.
        build(
            layout,
            &[0.3, 0.4],
            &[],
            &[],
            Placement::default(),
            &mut out,
        );
        assert!(
            out.iter().all(|s| s.width > 0.0),
            "{layout:?}: a missing width falls back to a drawable one"
        );
    }
    // A single element has no polyline to draw and must not underflow.
    assert!(
        figure(SpectrumLayout::Polyline, &[0.5]).is_empty(),
        "one point is not a line"
    );

    // An empty spectrum leaves every element at zero rather than reading
    // stale values.
    let mut levels = vec![7.0; 4];
    downsample(&[], &mut levels);
    assert_eq!(levels, vec![0.0; 4]);
}
