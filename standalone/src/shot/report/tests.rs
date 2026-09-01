//! Unit coverage for the `--report` machinery (Plan 0061 Phase 4).
//!
//! **These are the tests the move exists for.** While this code lived in
//! `examples/shot.rs` none of it could be exercised: `#[test]` in an example
//! compiles but never runs, so the only thing standing behind a thousand lines of
//! table generation, gate classification and transient analysis was one
//! subprocess test asserting that the JSON's braces balanced (ADR-0073, and the
//! coverage gap Plan 0061 Phase 4b left open when it scoped that test down).
//!
//! Everything here is GPU-free. The one function that needs a device -
//! `build_family_report` - stays covered by `standalone/tests/shot_cli.rs`.

// Tests index, expect and panic freely; this is not the render path.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lmv_core::preset::{GateFlag, GateKind};
use lmv_core::render::CaptureImage;
use lmv_core::render::metrics::StepResponse;

use super::*;

fn gate(param: &str, source: &str, kind: GateKind) -> GateReport {
    GateReport {
        param: param.to_string(),
        flag: GateFlag {
            source: source.to_string(),
            kind,
        },
    }
}

fn preset_report(name: &str, gates: Vec<GateReport>) -> PresetReport {
    PresetReport {
        name: name.to_string(),
        reactivity: [0.5, 0.25, 0.125, 0.0625],
        reactivity_low: [0.4, 0.2, 0.1, 0.05],
        reactivity_footprint: [0.9375, 0.4688, 0.2344, 0.1172],
        animation: 0.75,
        drive: 0.625,
        rate: 0.0123,
        coverage: 0.5,
        level: 0.1234,
        geometry: None,
        transient: Transient {
            response: StepResponse {
                rise_frames: 7,
                fall_frames: 11,
            },
            rise_settled: true,
            fall_settled: false,
        },
        gates,
    }
}

/// A `+` marks an *unsettled* cell and nothing else. The suffix is the only
/// thing separating "the response arrived in 7 frames" from "it had not arrived
/// by frame 7", and the two are indistinguishable from the number alone.
#[test]
fn a_transient_cell_marks_exactly_when_the_segment_did_not_settle() {
    assert_eq!(transient_cell(7, true), "7");
    assert_eq!(transient_cell(7, false), "7+");
    assert_eq!(transient_cell(0, true), "0");
    assert_eq!(transient_cell(0, false), "0+");
}

/// The three counts are three different claims and must not be derived from each
/// other. `gate_counts` counts by kind precisely so that adding a fourth kind
/// cannot silently inflate one of the other three (ADR-0062).
#[test]
fn gate_counts_separate_dead_branches_from_ceilings_and_saturation() {
    let p = preset_report(
        "mixed",
        vec![
            gate("a", "onset > 0.5", GateKind::Select { always: true }),
            gate("b", "bass > 0.9", GateKind::Compare { always: false }),
            gate(
                "c",
                "clamp(bass, 0, 2)",
                GateKind::Clamp {
                    peak_fraction_of_bound: 0.3,
                },
            ),
            gate(
                "d",
                "clamp(mid, 0, 1)",
                GateKind::Saturated { occupancy: 0.95 },
            ),
        ],
    );
    let (dead, ceilings, saturated) = gate_counts(&p);
    // A Compare counts WITH the Selects: both say a branch never rendered.
    assert_eq!(dead, 2, "Select and Compare are both dead branches");
    assert_eq!(ceilings, 1, "only the unapproached Clamp is a ceiling");
    assert_eq!(saturated, 1, "a pinned clamp is its own finding");
}

/// A saturated clamp is the *opposite* finding from an unapproached one — one
/// says the ceiling does no work, the other says the binding stopped being a
/// function of the audio. Neither may be classified as a dead gate.
#[test]
fn a_saturated_clamp_is_neither_a_dead_gate_nor_an_unapproached_ceiling() {
    let sat = gate(
        "d",
        "clamp(mid, 0, 1)",
        GateKind::Saturated { occupancy: 1.0 },
    );
    assert!(!is_dead_gate(&sat));
    assert!(is_saturated(&sat));

    let ceil = gate(
        "c",
        "clamp(bass, 0, 2)",
        GateKind::Clamp {
            peak_fraction_of_bound: 0.1,
        },
    );
    assert!(!is_dead_gate(&ceil));
    assert!(!is_saturated(&ceil));
}

/// Each gate kind gets its own prefix and its own sentence, because the reader's
/// next action differs: a dead branch is a preset that never renders one of its
/// looks, a ceiling is cosmetic, a saturated clamp is a binding gone constant.
#[test]
fn a_gate_line_names_its_kind_its_param_and_its_consequence() {
    let sel = gate_line(
        "aurora",
        &gate("warp", "onset > 0.5", GateKind::Select { always: true }),
    );
    assert!(sel.starts_with("  GATE aurora.warp:"), "got {sel}");
    assert!(sel.contains("onset > 0.5"), "quotes the source: {sel}");
    assert!(
        sel.contains("never went false") && sel.contains("`else` branch never ran"),
        "an always-true select kills the else branch: {sel}"
    );

    let cmp = gate_line(
        "aurora",
        &gate(
            "beat_flash",
            "bass > 0.9",
            GateKind::Compare { always: false },
        ),
    );
    assert!(cmp.starts_with("  COMP aurora.beat_flash:"), "got {cmp}");
    assert!(
        cmp.contains("never went true") && cmp.contains("constant 0"),
        "an always-false comparison reads as a constant 0: {cmp}"
    );

    let ceil = gate_line(
        "aurora",
        &gate(
            "gain",
            "clamp(bass, 0, 2)",
            GateKind::Clamp {
                peak_fraction_of_bound: 0.42,
            },
        ),
    );
    assert!(ceil.starts_with("  CEIL aurora.gain:"), "got {ceil}");
    assert!(ceil.contains("42% of its upper bound"), "got {ceil}");

    let sat = gate_line(
        "aurora",
        &gate(
            "gain",
            "clamp(mid, 0, 1)",
            GateKind::Saturated { occupancy: 0.955 },
        ),
    );
    assert!(sat.starts_with("  SAT  aurora.gain:"), "got {sat}");
    assert!(sat.contains("96% of hops"), "rounds for display: {sat}");
}

/// The four band stimuli are one-hot: frame `i` raises band `i` and leaves the
/// others at their baseline. If they were not, a "reactivity" differential would
/// be measuring two bands at once and no column would mean what it says.
#[test]
fn the_band_stimuli_raise_exactly_one_band_each() {
    let frames = band_stimuli_at(FULL_LEVELS);
    assert_eq!(frames.len(), 4);
    let read = |f: &AnalysisFrame| [f.bass, f.mid, f.treb, f.onset];
    for (i, f) in frames.iter().enumerate() {
        let v = read(f);
        assert_eq!(v[i], FULL_LEVELS[i], "frame {i} raises band {i}");
        for (j, other) in v.iter().enumerate() {
            if j != i {
                assert_eq!(
                    *other, 0.0,
                    "frame {i} leaves band {j} at rest, got {other}"
                );
            }
        }
    }
}

/// `LOW_LEVELS` is the realistic-material reading and must sit strictly under
/// full scale on every band — the whole point of the second table is that the
/// *gap* between the two is the finding (ADR-0042).
#[test]
fn the_realistic_levels_sit_under_full_scale_on_every_band() {
    for (i, (low, full)) in LOW_LEVELS.iter().zip(FULL_LEVELS.iter()).enumerate() {
        assert!(
            *low > 0.0 && low < full,
            "band {i}: LOW_LEVELS {low} must be in (0, {full})"
        );
    }
}

/// The text report's header carries the tier, because every number under it is
/// measured at that tier and a report read against another one is comparing two
/// capacity budgets (ADR-0045).
#[test]
fn the_text_report_header_names_its_source_and_its_tier() {
    let out = text_report("--presets fixtures", &[], Tier::Floor);
    assert_eq!(
        out, "visual-QA report [--presets fixtures] tier floor\n",
        "an empty roster still emits exactly the header line"
    );

    let rich = text_report("embedded defaults", &[], Tier::Rich);
    assert!(rich.contains("tier rich"), "got {rich}");
}

/// The table body renders one row per preset, with the transient marks the cells
/// carry, which is otherwise unreachable from any test.
#[test]
fn the_text_report_emits_a_row_per_preset_with_its_transient_marks() {
    let fam = FamilyReport {
        system: SystemKind::Swarm,
        presets: vec![
            preset_report("alpha", vec![]),
            preset_report("beta", vec![]),
        ],
        pixel: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        shape: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam], Tier::Floor);

    assert!(out.contains("=== swarm (2 presets) ==="), "got:\n{out}");
    for name in ["alpha", "beta"] {
        assert!(out.contains(name), "row for {name} missing from:\n{out}");
    }
    // rise_settled = true, fall_settled = false on the fixture: the rise cell is
    // bare and the fall cell is marked, in the two `{:>5}` columns that end the
    // row.
    assert!(
        out.contains("    7   11+"),
        "the marked cell must be the fall, not the rise:\n{out}"
    );
    assert!(
        out.contains("every branch was taken"),
        "a gate-free family says so rather than printing nothing:\n{out}"
    );
}

/// The footprint block (Plan 0077 Phase 4, backlog 0088) prints beside the
/// mean columns rather than replacing them: the mean table's values are the
/// historical reading and stay in place, and the footprint values — the ones
/// that can see reactivity spent on a concentrated bloom halo — appear in
/// their own labeled block. JSON carries the same reading under its own key.
#[test]
fn the_footprint_reading_prints_beside_the_mean_columns_not_instead_of_them() {
    let fam = FamilyReport {
        system: SystemKind::StarPattern,
        presets: vec![preset_report("bloomy", vec![])],
        pixel: vec![vec![0.0]],
        shape: vec![vec![0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam], Tier::Floor);

    // The mean value and the footprint value are both present — the fixture's
    // two arrays are distinct on every band, so each number is attributable.
    assert!(out.contains("0.500"), "the mean bass column stays:\n{out}");
    assert!(
        out.contains("0.938"),
        "the footprint bass value prints in its own block:\n{out}"
    );
    assert!(
        out.contains("over the lit footprint"),
        "the block explains itself:\n{out}"
    );
    assert!(
        out.contains("footprint_diff"),
        "the block names its statistic so a reader can find the definition:\n{out}"
    );

    let json = render_json(
        "src",
        &[FamilyReport {
            system: SystemKind::StarPattern,
            presets: vec![preset_report("bloomy", vec![])],
            pixel: vec![vec![0.0]],
            shape: vec![vec![0.0]],
            near_dups: Vec::new(),
        }],
        Tier::Floor,
    );
    assert!(
        json.contains("\"reactivity_footprint\":{\"bass\":0.9375"),
        "JSON carries the footprint reading under its own key: {json}"
    );
    assert_eq!(
        json.matches('{').count(),
        json.matches('}').count(),
        "braces balance with the new key: {json}"
    );
}

/// The `geom` column appears exactly where a line seam produced a measurement
/// and nowhere else (Plan 0075 Phase 2, backlog 0070): a family with no
/// `LineRenderer` in it gets no header, no cell, no `-` — the fraction is a
/// number an author reads while tuning `scale`, and a fabricated cell on a
/// swarm would read as a finding about a seam that does not exist.
#[test]
fn the_geometry_column_appears_only_for_families_with_a_line_seam() {
    let fam = |system, presets| FamilyReport {
        system,
        presets,
        pixel: vec![vec![0.0]],
        shape: vec![vec![0.0]],
        near_dups: Vec::new(),
    };

    // A line family: the column and the measured value are printed.
    let mut line_preset = preset_report("rosette", vec![]);
    line_preset.geometry = Some(0.3492);
    let line_fam = fam(SystemKind::StarPattern, vec![line_preset]);
    let out = text_report("src", &[line_fam], Tier::Floor);
    // The header token, not the bare substring: "geometry" also appears in the
    // near-duplicate summary line every family prints.
    assert!(
        out.contains("geom\n"),
        "line family carries the header column:\n{out}"
    );
    assert!(
        out.contains("0.3492"),
        "the measured fraction is printed to four places:\n{out}"
    );
    assert!(
        out.contains("in-frame geometry fraction"),
        "the column explains itself:\n{out}"
    );

    // A family with no line seam: no header, no placeholder, no explainer.
    let swarm_fam = fam(SystemKind::Swarm, vec![preset_report("drift", vec![])]);
    let out = text_report("src", &[swarm_fam], Tier::Floor);
    assert!(
        !out.contains("geom\n") && !out.contains("in-frame geometry fraction"),
        "a family with no line seam must omit the column entirely:\n{out}"
    );

    // JSON mirrors the omission: the key exists exactly when the value does.
    let mut measured = preset_report("rosette", vec![]);
    measured.geometry = Some(0.5);
    let with = render_json(
        "src",
        &[fam(SystemKind::StarPattern, vec![measured])],
        Tier::Floor,
    );
    assert!(with.contains("\"in_frame_geometry\":0.5"), "{with}");
    let without = render_json(
        "src",
        &[fam(SystemKind::Swarm, vec![preset_report("drift", vec![])])],
        Tier::Floor,
    );
    assert!(!without.contains("in_frame_geometry"), "{without}");
    assert_eq!(
        with.matches('{').count(),
        with.matches('}').count(),
        "braces balance with the new key: {with}"
    );
}

/// The JSON report is machine-read, so its two top-level keys are a contract -
/// this is the claim `shot_cli`'s subprocess test makes, asserted here in-process
/// where it costs milliseconds instead of sweeping a preset library.
#[test]
fn the_json_report_carries_its_top_level_keys_and_escapes_its_source() {
    let out = render_json("a \"quoted\" source", &[], Tier::Floor);
    assert!(
        out.starts_with('{') && out.trim_end().ends_with('}'),
        "{out}"
    );
    assert!(out.contains("\"source\""), "{out}");
    assert!(out.contains("\"families\""), "{out}");
    assert!(
        out.contains("\\\"quoted\\\""),
        "the source string is JSON-escaped: {out}"
    );
    assert_eq!(
        out.matches('{').count(),
        out.matches('}').count(),
        "braces balance: {out}"
    );
}

/// `families` is genuinely plural: one object per family, keyed by system name.
#[test]
fn the_json_report_emits_one_family_object_per_system() {
    let fam = |system, name| FamilyReport {
        system,
        presets: vec![preset_report(name, vec![])],
        pixel: vec![vec![0.0]],
        shape: vec![vec![0.0]],
        near_dups: Vec::new(),
    };
    let out = render_json(
        "src",
        &[fam(SystemKind::Swarm, "a"), fam(SystemKind::Emitter, "b")],
        Tier::Floor,
    );
    assert!(out.contains(SystemKind::Swarm.as_str()), "{out}");
    assert!(out.contains(SystemKind::Emitter.as_str()), "{out}");
    assert_eq!(
        out.matches('{').count(),
        out.matches('}').count(),
        "braces still balance with two families: {out}"
    );
}

/// A capture shorter than the probe schedule must not panic or index out of
/// bounds — it answers zero-length segments, which is what an empty slice means.
#[test]
fn probe_response_survives_a_capture_shorter_than_the_probe_window() {
    let t = probe_response(&[]);
    assert_eq!(t.response.rise_frames, 0);
    assert_eq!(t.response.fall_frames, 0);
}

/// Plan 0076 Phase 4: reachability walks the `[layer]`'s bindings — its params
/// **and** its bindable `mix` — through the same `Binding` machinery as the
/// top level. ADR-0090 called this a property to verify at implementation
/// rather than new design, and this is the verification: a layer gate that
/// never fires must flag, labeled with its namespace, while a live top-level
/// gate on the same preset must not.
#[test]
fn a_dead_gate_on_a_layer_binding_flags_in_the_reachability_walk() {
    let preset = lmv_core::preset::Preset::from_toml_str(
        "system = \"fragment_field\"\n\
         [params]\nwarp = \"select(onset > 0.2, 0.6, 0.3)\"\n\
         [layer]\nsystem = \"swarm\"\njoin = \"over\"\n\
         mix = \"select(treb > 9.0, 1.0, 0.5)\"\n\
         [layer.params]\nzoom = \"select(bass > 9.0, 1.0, 0.4)\"\n",
    )
    .expect("layered probe preset loads");

    // Frames that take the top-level gate both ways and can never reach the
    // layer's impossible thresholds (band levels are 0..1-ish; 9.0 is not).
    let frames: Vec<AnalysisFrame> = (0..8)
        .map(|i| AnalysisFrame {
            bass: 0.4,
            mid: 0.3,
            treb: 0.5,
            onset: if i % 2 == 0 { 0.5 } else { 0.0 },
            ..Default::default()
        })
        .collect();

    let gates = probe_reachability(&preset, &frames);
    let names: Vec<&str> = gates.iter().map(|g| g.param.as_str()).collect();
    assert!(
        names.contains(&"[layer] zoom"),
        "the dead layer-param gate must flag, labeled with its namespace: {names:?}"
    );
    assert!(
        names.contains(&"[layer] mix"),
        "the dead mix gate must flag: {names:?}"
    );
    assert!(
        !names.contains(&"warp"),
        "the live top-level gate must not flag: {names:?}"
    );
}

/// A flat RGBA image whose every channel holds `value` — the only fixture
/// [`mean_consecutive_diff`] needs, since `frame_diff` is a per-channel mean and
/// a uniform frame makes the expected answer exact rather than approximate.
fn flat(value: u8) -> CaptureImage {
    CaptureImage {
        width: 4,
        height: 4,
        rgba: vec![value; 4 * 4 * 4],
    }
}

/// `rate` is the one statistic in this module that walks the sequence, so its
/// defining property is that **order matters** — every other column here
/// differences two frames a fixed count apart and is blind to what happened
/// between them (ADR-0134).
///
/// A uniform ramp of `step` per frame must read back exactly `step / 255`, and
/// the same frames dealt in another order must not.
#[test]
fn the_rate_reading_is_the_mean_of_consecutive_frame_differences() {
    let step = 3u8;
    let ramp: Vec<CaptureImage> = (0..8).map(|i| flat(10 + i * step)).collect();
    let expected = step as f32 / 255.0;
    assert!(
        (mean_consecutive_diff(&ramp) - expected).abs() < 1e-6,
        "a constant per-frame step must come back as the mean: got {}, want {expected}",
        mean_consecutive_diff(&ramp)
    );

    // The same eight frames, reordered, so the hops are not all `step` and a
    // statistic that reads consecutive pairs has to answer differently — a
    // first-against-last differential would be identical for any permutation
    // that kept its endpoints.
    let shuffled: Vec<CaptureImage> = [0usize, 2, 1, 3, 5, 4, 6, 7]
        .iter()
        .map(|&i| flat(10 + (i as u8) * step))
        .collect();
    assert!(
        (mean_consecutive_diff(&shuffled) - expected).abs() > 1e-4,
        "reordering the frames must change the reading, got {} either way",
        mean_consecutive_diff(&shuffled)
    );

    // Degenerate inputs answer "no motion" rather than dividing by zero.
    assert_eq!(mean_consecutive_diff(&[]), 0.0);
    assert_eq!(mean_consecutive_diff(&[flat(7)]), 0.0);
}

/// `probe_rate` reads a **fixed window** of the probe schedule — the settled
/// tail of the loud plateau — so it is sensitive to *where* motion sits in the
/// capture, not only to how much of it there is.
///
/// The fixture puts a known step in that tail and holds every other region
/// still; reversing the same frames moves other regions into the window, so the
/// reading must change. (Reversal alone cannot change [`mean_consecutive_diff`]:
/// `frame_diff` is symmetric, so a reversed sequence has the identical multiset
/// of consecutive pairs. What reversal changes here is which frames the window
/// selects.)
#[test]
fn probe_rate_reads_the_settled_tail_of_the_loud_plateau_and_not_the_rest() {
    let plateau_end = PROBE_PRE + PROBE_WINDOW;
    let tail_start = plateau_end - RATE_TAIL;
    let total = PROBE_PRE + 2 * PROBE_WINDOW;
    let step = 3u8;
    let seq: Vec<CaptureImage> = (0..total)
        .map(|i| {
            if i < tail_start {
                flat(10)
            } else if i < plateau_end {
                // Only the tail ramps, and at a known rate.
                flat(10 + (i - tail_start) as u8 * step)
            } else {
                // The fall segment sits somewhere else entirely, and still.
                flat(200)
            }
        })
        .collect();
    let expected = step as f32 / 255.0;
    assert!(
        (probe_rate(&seq) - expected).abs() < 1e-6,
        "the tail's own step is the reading: got {}, want {expected}",
        probe_rate(&seq)
    );

    let mut reversed = seq;
    reversed.reverse();
    assert!(
        (probe_rate(&reversed) - expected).abs() > 1e-4,
        "reversing moves other frames into the window, so the reading must change: got {}",
        probe_rate(&reversed)
    );

    // A capture shorter than the schedule yields an empty window, not a panic.
    assert_eq!(probe_rate(&[]), 0.0);
}

/// The `rate` cell is marked from the probe's **rise**: an unsettled rise means
/// the frames it averaged were still travelling toward the plateau, so the
/// number describes the transient rather than the steady motion.
#[test]
fn a_rate_cell_marks_exactly_when_the_rise_did_not_settle() {
    assert_eq!(rate_cell(0.0123, true), "0.0123");
    assert_eq!(rate_cell(0.0123, false), "0.0123+");
    assert_eq!(rate_cell(0.0, true), "0.0000");
    assert_eq!(rate_cell(0.0, false), "0.0000+");
}

/// Both motion readings print in the table and both reach `--json`, and the
/// JSON carries the size `rate` was measured at rather than leaving a consumer
/// to assume it matches the columns beside it (ADR-0134).
#[test]
fn the_motion_columns_print_in_the_table_and_carry_their_size_in_the_json() {
    let fam = || FamilyReport {
        system: SystemKind::Swarm,
        presets: vec![preset_report("drifter", vec![])],
        pixel: vec![vec![0.0]],
        shape: vec![vec![0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam()], Tier::Floor);
    assert!(out.contains("drive"), "the drive header prints:\n{out}");
    assert!(out.contains("rate"), "the rate header prints:\n{out}");
    assert!(out.contains("0.625"), "the drive value prints:\n{out}");
    // `rise_settled` is true on the fixture, so this cell is bare.
    assert!(out.contains("0.0123"), "the rate value prints:\n{out}");
    assert!(
        !out.contains("0.0123+"),
        "a settled rise leaves the rate cell unmarked:\n{out}"
    );
    assert!(
        out.contains("NEITHER IS A THRESHOLD") && out.contains("anchor"),
        "the anchoring caveat rides beside the columns, not in a footnote:\n{out}"
    );

    let json = render_json("src", &[fam()], Tier::Floor);
    assert!(json.contains("\"drive\":0.625"), "{json}");
    assert!(
        json.contains(&format!(
            "\"rate\":{{\"mean\":0.0123,\"settled\":true,\"measured_at_px\":{PROBE_SIZE}}}"
        )),
        "the rate object carries its own size: {json}"
    );
    assert_eq!(
        json.matches('{').count(),
        json.matches('}').count(),
        "braces balance with the new keys: {json}"
    );
}

/// The level column reaches both outputs, one cell per preset (ADR-0150).
///
/// Its four decimals are not decoration: the statistic is a comparison number
/// and the comparisons it is read for are small. Three places would round a
/// 5 % move between two neighbouring rows to nothing, which is the resolution
/// the encoded mean it replaces already had.
#[test]
fn the_level_column_prints_per_preset_and_reaches_the_json() {
    let fam = || FamilyReport {
        system: SystemKind::Swarm,
        presets: vec![
            preset_report("drifter", vec![]),
            preset_report("eddy", vec![]),
        ],
        pixel: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        shape: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam()], Tier::Floor);
    assert!(out.contains("level"), "the level header prints:\n{out}");
    assert_eq!(
        out.matches("0.1234").count(),
        2,
        "one level cell per preset, at four decimals:\n{out}"
    );

    let json = render_json("src", &[fam()], Tier::Floor);
    assert_eq!(
        json.matches("\"level\":0.1234").count(),
        2,
        "the level reaches the json once per preset: {json}"
    );
}

/// The widest possible row — a line family, so the `geom` column is present too
/// — still fits a 100-column terminal with both new columns in place. The
/// content lane reads this table many times a session, and a wrapped row stops
/// lining up with its header.
#[test]
fn no_report_table_line_wraps_at_a_hundred_columns() {
    let mut line_preset = preset_report("Star Mandala Bordered", vec![]);
    line_preset.geometry = Some(0.3492);
    let fam = FamilyReport {
        system: SystemKind::StarPattern,
        presets: vec![line_preset],
        pixel: vec![vec![0.0]],
        shape: vec![vec![0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam], Tier::Floor);
    // The explanatory prose blocks wrap on their own and have no columns to
    // line up; the claim is about the header and the rows under it. Both are
    // found by content rather than by position, so a block inserted between
    // them later cannot silently drop this test's coverage. The row is found by
    // the label the fitter produces, not by the raw name.
    let label = fit_name("Star Mandala Bordered");
    let table: Vec<&str> = out
        .lines()
        .filter(|l| l.contains(&label) || l.trim_start().starts_with("preset "))
        .collect();
    assert_eq!(
        table.len(),
        6,
        "all three tables carry a header and this preset's row:\n{out}"
    );
    for line in table {
        assert!(
            line.chars().count() <= 100,
            "a table line is {} columns wide:\n{line}",
            line.chars().count()
        );
    }
}

/// design-backlog 0131: two presets whose display names share their first
/// [`NAME_WIDTH`] characters must print as **distinguishable** rows in all three
/// tables. Constructed as an explicit pair rather than leaned on the shipped
/// library, so curating the colliding preset away cannot silently retire this.
#[test]
fn two_names_sharing_their_first_fourteen_characters_print_as_distinct_rows() {
    // The live collision at the time of writing: `Tiled Rosette` is 13
    // characters, so a 14-wide tail truncation pads it to exactly what it
    // truncates `Tiled Rosette Mono` to.
    let fam = FamilyReport {
        system: SystemKind::FragmentField,
        presets: vec![
            preset_report("Tiled Rosette", vec![]),
            preset_report("Tiled Rosette Mono", vec![]),
        ],
        pixel: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        shape: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        near_dups: Vec::new(),
    };
    let out = text_report("src", &[fam], Tier::Floor);

    // Every table prints two rows, and in each table the two labels differ.
    let labels: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("  Tiled"))
        .map(|l| l.get(2..2 + NAME_WIDTH).unwrap_or(l))
        .collect();
    assert_eq!(labels.len(), 6, "three tables, two rows each:\n{out}");
    for pair in labels.chunks(2) {
        assert_ne!(
            pair.first(),
            pair.last(),
            "the two rows print the same label:\n{out}"
        );
    }
    // And every cell is still exactly the column width, so the numbers line up.
    for label in &labels {
        assert_eq!(label.chars().count(), NAME_WIDTH, "got {label:?}");
    }
}

/// The fitter elides the **middle**, because the distinguishing part of a name
/// in this library is its tail (`Mono`, `Gallery`, `Bordered`). A name that fits
/// is passed through untouched, so no historical row label moves.
#[test]
fn fit_name_keeps_short_names_whole_and_elides_the_middle_of_long_ones() {
    assert_eq!(fit_name("Whorl"), "Whorl");
    assert_eq!(fit_name("Thomas Gallery"), "Thomas Gallery");
    assert_eq!(fit_name("Thomas Gallery").chars().count(), NAME_WIDTH);

    let long = fit_name("Star Mandala Bordered");
    assert_eq!(long.chars().count(), NAME_WIDTH, "got {long:?}");
    assert!(long.contains('~'), "the elision is marked: {long:?}");
    assert!(long.starts_with("Star Ma"), "the head survives: {long:?}");
    assert!(long.ends_with("rdered"), "the tail survives: {long:?}");
}

/// The whole shipped library fits into distinct row labels. This is the claim
/// the report is actually read under — a fitter that resolved the one known pair
/// and collided somewhere else would pass the pair test above and still print an
/// ambiguous table.
#[test]
fn every_shipped_preset_name_fits_to_a_distinct_label() {
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for preset in lmv_core::preset::default_presets() {
        let label = fit_name(&preset.name);
        assert!(
            label.chars().count() <= NAME_WIDTH,
            "{:?} fits to {label:?}, wider than the column",
            preset.name
        );
        if let Some(other) = seen.insert(label.clone(), preset.name.clone()) {
            panic!("{:?} and {:?} both print as {label:?}", other, preset.name);
        }
    }
}
