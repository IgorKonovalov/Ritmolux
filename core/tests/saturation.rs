//! Clamp saturation (Plan 0056 Phase 3, HARD — ADR-0062).
//!
//! Every other reactivity instrument in this project compares a driven band
//! against **silence**, and a binding that saturates just above the noise floor
//! scores perfectly on that question while failing the real one. It is a binary
//! switch, and to a silence-relative test a binary switch is maximally reactive.
//! Plan 0048 Phase 7 measured the consequence: 263 of 332 clamped band terms
//! pinned at the real-music median, 14 presets with no live audio term at all,
//! and a fully green suite the whole time.
//!
//! This gate closes that. It walks every shipped preset's expression trees over
//! the same 12 s `dynamic:110` probe `--report` uses, records the fraction of
//! hops each `clamp()` spends **at** its upper bound, and fails on any that
//! never lets go. A preset whose clamp is genuinely a safety rail declares
//! `[occupancy] exempt = [...]` — which silences this gate and nothing else: the
//! binding still appears in `--report`'s `occ` count and `SAT` lines.
//!
//! **CPU only.** No renderer, no adapter, no skip path: this is a walk over
//! compiled expressions, so it holds on every runner.
//!
//! The threshold ([`SATURATED_OCCUPANCY`]) is measured, not reasoned to — this
//! test prints the whole library's distribution on every run, which is where the
//! next re-measurement comes from.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, Analyzer, HOP_SIZE, WARMUP_HOPS};
use lmv_core::preset::{
    Expr, GateKind, Observations, Preset, SATURATED_OCCUPANCY, Variables, default_presets,
};

/// BPM, seconds and format of the probe. Identical to `shot --report`'s
/// reachability probe on purpose: a binding this gate fails must be the same
/// binding the report names, or the failure sends its reader to the wrong file.
const BPM: f32 = 110.0;
const SECS: f32 = 12.0;
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// Hops skipped before the walk starts. Until the long window fills, every band
/// reads zero, so a clamp would look *unoccupied* on evidence that is only the
/// analyzer starting up. `+ 4` matches `shot`'s `FILMSTRIP_WARMUP`.
const WARMUP: usize = WARMUP_HOPS + 4;

/// Positions a per-element binding's `index` is sampled at, matching the report.
const INDEX_SAMPLES: usize = 5;

/// The probe: the real analyzer over the real generator, so bands move together
/// the way music's do rather than the way a hand-built frame does.
fn probe_frames() -> Vec<AnalysisFrame> {
    let pcm = lmv_core::signal::dynamic_groove(BPM, SECS, FORMAT);
    let mut analyzer = Analyzer::new(FORMAT).expect("probe analyzer");
    let hop_samples = HOP_SIZE * FORMAT.channels as usize;
    let mut frames = Vec::new();
    for (index, hop) in pcm.chunks(hop_samples).enumerate() {
        analyzer.push_interleaved(hop);
        let frame = analyzer.take_frame();
        if index >= WARMUP {
            frames.push(frame);
        }
    }
    assert!(!frames.is_empty(), "the probe produced no analysis frames");
    frames
}

/// Seconds per hop — the clock `time` is derived from, since there is no render
/// loop here.
fn hop_seconds() -> f32 {
    HOP_SIZE as f32 / FORMAT.sample_rate as f32
}

/// Evaluate `expr` over `frames` into `obs`, exactly as `--report`'s walk does:
/// through the engine's own frame binding, salted with the **pinned** salt
/// (ADR-0051 — this is a capture path), and sampled across `index` for a
/// per-element binding.
fn drive(expr: &Expr, frames: &[AnalysisFrame], salt: u32, obs: &mut Observations) {
    for (hop, frame) in frames.iter().enumerate() {
        let vars = Variables::from_frame(frame, hop as f32 * hop_seconds()).with_salt(salt);
        if expr.uses_index() {
            for step in 0..INDEX_SAMPLES {
                let t = step as f32 / (INDEX_SAMPLES.max(2) - 1) as f32;
                expr.eval_probed(&vars.with_index(t), obs);
            }
        } else {
            expr.eval_probed(&vars, obs);
        }
    }
}

/// The highest occupancy any `clamp()` in this expression reached, or `None` if
/// it holds no clamp the walk ever evaluated. This is the raw statistic, below
/// the flag's threshold — it is what the distribution is made of.
fn peak_occupancy(obs: &Observations) -> Option<f32> {
    obs.nodes()
        .iter()
        .filter(|n| matches!(n, lmv_core::preset::NodeObservation::Clamp { .. }))
        .map(|n| n.occupancy())
        .fold(None, |acc, occ| Some(acc.map_or(occ, |a: f32| a.max(occ))))
}

/// Whether `expr`'s clamp rendering as `source` was at its bound on this one
/// frame.
///
/// Over a single hop occupancy is `0.0` or `1.0`, so a `Saturated` flag from a
/// fresh `Observations` **is** the per-hop answer — no new observation field is
/// needed to ask it.
fn pinned_on(expr: &Expr, frame: &AnalysisFrame, salt: u32, source: &str) -> bool {
    let mut obs = Observations::new();
    let vars = Variables::from_frame(frame, 0.0).with_salt(salt);
    if expr.uses_index() {
        expr.eval_probed(&vars.with_index(0.0), &mut obs);
    } else {
        expr.eval_probed(&vars, &mut obs);
    }
    expr.flag_gates(&obs)
        .iter()
        .any(|f| matches!(f.kind, GateKind::Saturated { .. }) && f.source == source)
}

/// The quietest hop of the probe at which this clamp was **still** at its bound.
///
/// This is the "reached at" level, measured rather than derived: inverting an
/// arbitrary expression to solve for its input is not possible in general, but
/// asking the probe which of its hops still pinned the bound is, and it answers
/// in the units an author writes gains in. When the answer is the probe's
/// quietest hop, the ceiling is reached at near-silence and the gain is the
/// whole defect.
fn ceiling_reached_at(
    expr: &Expr,
    frames: &[AnalysisFrame],
    salt: u32,
    source: &str,
) -> Option<[f32; 4]> {
    let mut quietest: Option<(f32, [f32; 4])> = None;
    for frame in frames {
        if !pinned_on(expr, frame, salt, source) {
            continue;
        }
        let levels = [frame.bass, frame.mid, frame.treb, frame.onset];
        let loudness = levels.iter().sum::<f32>();
        if quietest.is_none_or(|(seen, _)| loudness < seen) {
            quietest = Some((loudness, levels));
        }
    }
    quietest.map(|(_, levels)| levels)
}

/// One row of the measured distribution: a binding that holds at least one
/// `clamp()`, and the highest occupancy any of them reached.
struct Row {
    preset: String,
    param: String,
    occupancy: f32,
    exempt: bool,
}

/// Walk one preset, returning its distribution rows and its failures.
///
/// The walk covers the `[layer]`'s bindings too — its params and its bindable
/// `mix` — since Plan 0076 Phase 4: they are the same `Binding` machinery, and
/// a layer clamp pinned at its bound is the same defect in another namespace.
/// Layer rows are labeled `[layer] <param>`; an `[occupancy] exempt` entry
/// matches the **raw** param name in either namespace, since the table
/// predates layers and names params, not namespaces.
fn walk_preset(preset: &Preset, frames: &[AnalysisFrame]) -> (Vec<Row>, Vec<String>) {
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let layer_bindings = preset
        .layer
        .iter()
        .flat_map(|layer| layer.params.iter().chain(layer.mix.iter()));
    let bindings = preset
        .params
        .iter()
        .map(|binding| (binding, false))
        .chain(layer_bindings.map(|binding| (binding, true)));
    for (binding, in_layer) in bindings {
        let display = if in_layer {
            format!("[layer] {}", binding.name)
        } else {
            binding.name.clone()
        };
        let mut obs = Observations::new();
        drive(&binding.expr, frames, preset.pinned_salt, &mut obs);
        let exempt = preset.occupancy_exempt.contains(&binding.name);
        if let Some(occupancy) = peak_occupancy(&obs) {
            rows.push(Row {
                preset: preset.name.clone(),
                param: display.clone(),
                occupancy,
                exempt,
            });
        }
        for flag in binding.expr.flag_gates(&obs) {
            let GateKind::Saturated { occupancy } = flag.kind else {
                continue;
            };
            // The exemption silences the gate and nothing else — the flag was
            // still produced above, and `--report` still prints it.
            if exempt {
                continue;
            }
            let reached = ceiling_reached_at(
                &binding.expr,
                frames,
                preset.pinned_salt,
                &flag.source,
            )
            .map_or_else(
                || "somewhere in the probe".to_string(),
                |[bass, mid, treb, onset]| {
                    format!(
                        "already at bass {bass:.3} mid {mid:.3} treb {treb:.3} onset {onset:.3}"
                    )
                },
            );
            failures.push(format!(
                "{}.{}: `{}` sat at its upper bound on {:.0}% of hops — the ceiling is \
                 {reached}, so this bound is a gain rather than a limit and the binding \
                 reads as the constant its ceiling is. Divide the inner gain until the \
                 bound is reached only on peaks, or declare `[occupancy] exempt = [\"{}\"]` \
                 if pinning is the design",
                preset.name,
                display,
                flag.source,
                occupancy * 100.0,
                binding.name,
            ));
        }
    }
    (rows, failures)
}

/// Print the measured distribution — the input to the next re-measurement of
/// [`SATURATED_OCCUPANCY`], which is a constant with a shelf life.
fn print_distribution(rows: &mut [Row]) {
    rows.sort_by(|a, b| b.occupancy.total_cmp(&a.occupancy));
    let buckets = [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.01];
    println!(
        "clamp occupancy across {} clamped binding(s), threshold {SATURATED_OCCUPANCY}:",
        rows.len()
    );
    for pair in buckets.windows(2) {
        let (lo, hi) = (
            pair.first().copied().unwrap_or(0.0),
            pair.get(1).copied().unwrap_or(1.01),
        );
        let n = rows
            .iter()
            .filter(|r| r.occupancy >= lo && r.occupancy < hi)
            .count();
        println!("  [{lo:.2}, {hi:.2}) {n:>4}");
    }
    println!("  highest 12:");
    for row in rows.iter().take(12) {
        let mark = if row.exempt { " (exempt)" } else { "" };
        println!(
            "    {:.4}  {}.{}{mark}",
            row.occupancy, row.preset, row.param
        );
    }
}

#[test]
fn no_shipped_clamp_sits_at_its_ceiling() {
    let frames = probe_frames();
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for preset in default_presets() {
        let (mut r, mut f) = walk_preset(&preset, &frames);
        rows.append(&mut r);
        failures.append(&mut f);
    }
    print_distribution(&mut rows);
    assert!(
        !rows.is_empty(),
        "the shipped library holds no clamps at all, so this gate is measuring nothing"
    );
    assert!(
        failures.is_empty(),
        "these bindings are saturated — a clamp pinned at its bound is a constant, not a \
         parameter:\n{}",
        failures.join("\n")
    );
}

/// A preset whose gain is written for full scale and met with normalized levels
/// — the Plan 0048 Phase 7 defect, reduced to one binding.
const OVERDRIVEN: &str = r#"
system = "fragment_field"
name   = "Overdriven"

[params]
warp = "clamp(bass * 16, 0, 0.3)"
"#;

/// The same preset, declaring that the pin is the design.
const OVERDRIVEN_EXEMPT: &str = r#"
system = "fragment_field"
name   = "Overdriven Exempt"

[params]
warp = "clamp(bass * 16, 0, 0.3)"

[occupancy]
exempt = ["warp"]
"#;

#[test]
fn an_over_driven_binding_fails_with_the_fix_in_the_message() {
    let frames = probe_frames();
    let preset = Preset::from_toml_str(OVERDRIVEN).expect("fixture parses");
    let (_, failures) = walk_preset(&preset, &frames);
    let message = failures.first().expect("the over-driven binding must fail");
    assert!(
        failures.len() == 1,
        "one binding, one failure: {failures:#?}"
    );
    // Naming the binding is the property that makes occupancy worth building:
    // a frame differential can only say "this preset is flat".
    assert!(
        message.contains("Overdriven.warp"),
        "the failure must name the binding: {message}"
    );
    assert!(
        message.contains("clamp(bass * 16, 0, 0.3)"),
        "the failure must quote the clamp: {message}"
    );
    // And the level it is already reached at, so the fix is arithmetic rather
    // than a bisection.
    assert!(
        message.contains("the ceiling is already at bass "),
        "the failure must state the level the ceiling is reached at: {message}"
    );
}

#[test]
fn an_in_file_exemption_silences_the_gate_but_not_the_diagnostic() {
    let frames = probe_frames();
    let preset = Preset::from_toml_str(OVERDRIVEN_EXEMPT).expect("fixture parses");
    assert_eq!(
        preset.occupancy_exempt,
        vec!["warp".to_string()],
        "the [occupancy] table is read at load"
    );

    let (rows, failures) = walk_preset(&preset, &frames);
    assert!(
        failures.is_empty(),
        "an exempted clamp must not fail the gate: {failures:#?}"
    );

    // The diagnostic half is untouched: the walk still produces the flag, which
    // is what `--report` prints. An exemption is a place to hide, and staying
    // visible in review is the only mitigation there is (ADR-0062).
    let binding = preset
        .params
        .iter()
        .find(|b| b.name == "warp")
        .expect("the fixture binds warp");
    let mut obs = Observations::new();
    drive(&binding.expr, &frames, preset.pinned_salt, &mut obs);
    assert!(
        binding
            .expr
            .flag_gates(&obs)
            .iter()
            .any(|f| matches!(f.kind, GateKind::Saturated { .. })),
        "the exemption must not suppress the flag itself"
    );
    assert!(
        rows.iter().any(|r| r.param == "warp" && r.exempt),
        "an exempted binding still carries its measured occupancy"
    );
}

#[test]
fn an_exemption_naming_nothing_warns_rather_than_passing_silently() {
    // A typo here would leave an author believing a gate was exempted while it
    // goes on failing on the real name — the same reasoning `[smoothing]` uses.
    let preset = Preset::from_toml_str(
        r#"
system = "fragment_field"
name   = "Typo"

[params]
warp = "clamp(bass * 16, 0, 0.3)"

[occupancy]
exempt = ["wrap"]
"#,
    )
    .expect("parses");
    assert!(
        preset.warnings.iter().any(|w| w.contains("wrap")),
        "an inert exemption must be surfaced: {:?}",
        preset.warnings
    );
}
