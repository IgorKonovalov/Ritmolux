//! The `shot --report` machinery: the reactivity / animation / distinctness
//! tables, the expression-reachability pass, and the transient probe.
//!
//! **It lives in the library rather than in `examples/shot.rs` so its `#[test]`s
//! actually run** (Plan 0061 Phase 4). A `#[test]` inside an example is compiled
//! but never executed by `cargo test` or `cargo nextest run`, so for as long as
//! this code sat in the example the only thing exercising it was one subprocess
//! CLI invocation asserting that its JSON had balanced braces.
//!
//! The example keeps argument handling, the image-writing capture modes and file
//! I/O. Nothing here names a type from the `image` crate - that is the
//! ADR-0011 / ADR-0033 boundary, and it is what lets this module be an ordinary
//! part of the library while `image` stays a dev-dependency.

use std::fmt::Write as _;

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, Analyzer, HOP_SIZE, SPECTRUM_BINS};
use lmv_core::preset::{
    GateFlag, GateKind, Observations, Preset, SATURATED_OCCUPANCY, SystemKind, Variables,
};
use lmv_core::render::metrics::{
    StepResponse, coverage, frame_diff, quadrant_spread, segment_settled, step_response,
    struct_diff,
};
use lmv_core::render::scenes::lines::renderer::{set_extent_diagnostic, take_draw_extent};
use lmv_core::render::{CaptureImage, Renderer, Tier};

use crate::shot::film::FILMSTRIP_WARMUP;
use crate::shot::json::{json_matrix, json_string, json_transient, num};

/// Report render size — small; the metrics don't need resolution.
const REPORT_SIZE: u32 = 192;
const REPORT_FRAMES: u32 = 24;
const REPORT_FRAMES_LATE: u32 = 48;
const NEAR_DUP_STRUCT: f32 = 0.08;
const COVERAGE_EPS: u8 = 10;

/// Render size for the transient probe, deliberately smaller than
/// [`REPORT_SIZE`]. The probe reads the GPU back **once per frame** rather than
/// once per capture, so its cost is dominated by readbacks in a way no other
/// column is — and what it measures is temporal, not spatial: rounding the
/// settle frame is unaffected by resolution, while quartering the pixels is
/// most of what keeps a full-library `--report` in the same time bracket it has
/// always been in.
const PROBE_SIZE: u32 = 96;
/// Frames of silence before the step, so the response starts settled.
const PROBE_PRE: usize = 6;
/// Frames held on each side of the step. The rise and fall windows are the
/// **same length** on purpose (see `step_response`): each is normalized against
/// its own final frame, so unequal windows would give the two directions
/// different truncation bias.
///
/// **48 frames is 0.8 s, and a great many presets do not settle inside it.**
/// This comment used to say such a response "reads *clamped* rather than
/// measured — the asymmetry still shows, the magnitude understates", and every
/// clause of that was wrong (corrected by Plan 0038 Phase 8). Nothing is
/// clamped: `frames_to_settle` normalizes against the segment's own last frame,
/// so a still-travelling response supplies a short total, crosses every
/// threshold early, and returns a **plausible smaller number** with no signal
/// that anything went wrong. The bias is uneven across thresholds, so the shape
/// is distorted toward *even* as well — which is how a truncated window once
/// falsified an ADR here.
///
/// The arithmetic: staying inside [`PROBE_SETTLE_TOL`] needs about
/// `0.8 / ln(50)` — a release above roughly **0.2 s** is already truncated, and
/// most of the `{ attack, release }` pairs in the shipped set are well above it.
///
/// Lengthening this is a direct multiplier on the report's wall clock, so the
/// fix is not a wider window — it is that a truncated cell is **marked** rather
/// than published bare. See [`probe_response`].
const PROBE_WINDOW: usize = 48;

/// Fraction of a segment's travel that may still be unfinished at its last frame
/// before its transient cell is marked. Matches the easing suite's own gate.
const PROBE_SETTLE_TOL: f32 = 0.02;

struct PresetReport {
    name: String,
    reactivity: [f32; 4], // bass, mid, treb, onset
    /// The same four differentials measured under [`band_stimuli_low`]. Stored
    /// beside the full-scale triple rather than replacing it — the pair is the
    /// reading (ADR-0042).
    reactivity_low: [f32; 4],
    animation: f32,
    coverage: f32,
    /// The in-frame geometry fraction of the fully-driven capture's last frame
    /// (Plan 0075 Phase 2, design-backlog 0070): the share of drawn segment
    /// length inside the render target, measured at `LineRenderer::draw`
    /// (ADR-0083). `None` for the families that build no segment list — the
    /// column is omitted rather than printed as a fake number.
    ///
    /// **A number to read while tuning `scale`, never a threshold**: ADR-0083
    /// twice proved no absolute value orders the library (two shipped presets
    /// deliberately leave the frame and bracket a frozen defect). This puts the
    /// fraction where the over-scale defect class is actually introduced —
    /// under the author's eyes — which is the only place it can be caught for
    /// new content.
    geometry: Option<f32>,
    transient: Transient,
    /// Gates this preset's expressions never exercised under the realistic
    /// probe — suspects, not convictions (see [`probe_reachability`]).
    gates: Vec<GateReport>,
}

/// One flagged gate, together with the binding it came from.
#[derive(Clone)]
struct GateReport {
    /// The parameter whose expression holds the gate.
    param: String,
    flag: GateFlag,
}

struct FamilyReport {
    system: SystemKind,
    presets: Vec<PresetReport>,
    pixel: Vec<Vec<f32>>,
    shape: Vec<Vec<f32>>,
    near_dups: Vec<(String, String)>,
}

/// Render the `--report` tables for `presets` and print them.
///
/// Takes the three `Args` fields it actually reads rather than `Args` itself:
/// the CLI's argument struct stays in the example, so the library does not grow
/// a dependency on the shape of a command line.
pub fn run(
    presets: Vec<Preset>,
    source: &str,
    tier: Tier,
    family: Option<SystemKind>,
    json: bool,
) -> Result<(), String> {
    let meta: Vec<(String, SystemKind)> =
        presets.iter().map(|p| (p.name.clone(), p.system)).collect();
    // The structural pass runs first: it reads the compiled expressions, and the
    // renderer is about to take ownership of them.
    let gates = reachability_pass(&presets)?;
    let mut r = super::renderer(REPORT_SIZE, REPORT_SIZE, presets, tier)?;

    // The roster, not a copy of it: a hand-maintained list here silently omitted
    // a new system from the report instead of failing to build.
    let mut reports = Vec::new();
    for system in SystemKind::ALL {
        if family.is_some_and(|f| f != system) {
            continue;
        }
        let names: Vec<String> = meta
            .iter()
            .filter(|(_, s)| *s == system)
            .map(|(n, _)| n.clone())
            .collect();
        if names.is_empty() {
            continue;
        }
        reports.push(build_family_report(&mut r, system, &names, &gates)?);
    }

    if json {
        print!("{}", render_json(source, &reports, tier));
    } else {
        print!("{}", text_report(source, &reports, tier));
    }
    Ok(())
}

fn build_family_report(
    r: &mut Renderer,
    system: SystemKind,
    names: &[String],
    gates: &[(String, Vec<GateReport>)],
) -> Result<FamilyReport, String> {
    let silent = AnalysisFrame::default();
    let loud = loud_frame();
    let bands = band_stimuli();
    let bands_low = band_stimuli_low();

    // The transient probe runs first and at its own smaller size, so the resize
    // happens twice per family rather than twice per preset.
    r.resize(PROBE_SIZE, PROBE_SIZE);
    let stimulus = step_stimulus();
    let mut transients = Vec::with_capacity(names.len());
    for name in names {
        let images = r
            .capture_preset_over(name, &stimulus)
            .map_err(|e| format!("probe `{name}`: {e}"))?;
        transients.push(probe_response(&images));
    }
    r.resize(REPORT_SIZE, REPORT_SIZE);

    let mut presets = Vec::new();
    let mut fixed_caps = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let base = capture(r, name, &silent, REPORT_FRAMES)?;
        let mut reactivity = [0.0f32; 4];
        let mut reactivity_low = [0.0f32; 4];
        for (i, frame) in bands.iter().enumerate() {
            let lit = capture(r, name, frame, REPORT_FRAMES)?;
            reactivity[i] = frame_diff(&base, &lit);
        }
        // Against the same silent baseline, so the two triples differ only in
        // the stimulus level.
        for (i, frame) in bands_low.iter().enumerate() {
            let lit = capture(r, name, frame, REPORT_FRAMES)?;
            reactivity_low[i] = frame_diff(&base, &lit);
        }
        let late = capture(r, name, &silent, REPORT_FRAMES_LATE)?;
        let animation = frame_diff(&base, &late);

        // The in-frame geometry diagnostic rides the fully-driven capture the
        // coverage column already takes, scoped to exactly that capture — it is
        // asserted byte-inert (`core/tests/geometry_extent.rs`), so the only
        // cost is a CPU loop over the segment list. `take_draw_extent` yields
        // the last drawn frame's measurement, and nothing at all for a preset
        // that drew through no line renderer — which is how the non-line
        // families report `None` without a second family roster here.
        set_extent_diagnostic(true);
        let fixed = capture(r, name, &loud, REPORT_FRAMES_LATE)?;
        let geometry = take_draw_extent().and_then(|extent| extent.fraction());
        set_extent_diagnostic(false);
        let bg = corner(&fixed);
        let cov = coverage(&fixed, bg, COVERAGE_EPS);
        // Belt-and-braces "not a dot" note folded into coverage via spread:
        // a single-quadrant frame is suspicious even at decent coverage.
        let _spread = quadrant_spread(&fixed, bg, COVERAGE_EPS);

        presets.push(PresetReport {
            name: name.clone(),
            reactivity,
            reactivity_low,
            animation,
            coverage: cov,
            geometry,
            transient: transients.get(index).copied().unwrap_or(Transient {
                response: StepResponse {
                    rise_frames: 0,
                    fall_frames: 0,
                },
                // A probe that never ran is not a settled measurement.
                rise_settled: false,
                fall_settled: false,
            }),
            gates: gates
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, g)| g.clone())
                .unwrap_or_default(),
        });
        fixed_caps.push(fixed);
    }

    // Pairwise pixel + shape matrices over the fixed-frame captures.
    let n = fixed_caps.len();
    let mut pixel = vec![vec![0.0f32; n]; n];
    let mut shape = vec![vec![0.0f32; n]; n];
    let mut near_dups = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let pd = frame_diff(&fixed_caps[i], &fixed_caps[j]);
            let sd = struct_diff(&fixed_caps[i], &fixed_caps[j]);
            pixel[i][j] = pd;
            shape[i][j] = sd;
            if i < j && sd < NEAR_DUP_STRUCT {
                near_dups.push((names[i].clone(), names[j].clone()));
            }
        }
    }

    Ok(FamilyReport {
        system,
        presets,
        pixel,
        shape,
        near_dups,
    })
}

fn capture(
    r: &mut Renderer,
    name: &str,
    frame: &AnalysisFrame,
    frames: u32,
) -> Result<CaptureImage, String> {
    r.capture_preset(name, frame, frames)
        .map_err(|e| format!("capture `{name}`: {e}"))
}

/// The log-band index ranges each named band roughly occupies, so a report
/// stimulus lights the part of the `spectrum` array its own scalar summarises.
/// Mirrors `core/tests/reactivity.rs`: a frame claiming `bass = 1.0` over 64
/// silent log-bands is not a frame any audio could produce, and under it a
/// preset reading the array through `bin()` — or the whole `spectrum` system —
/// would correctly draw nothing, so the report would be scoring the fixture
/// rather than the preset.
///
/// **Approximate**, derived from the ~20 Hz-Nyquist log spacing and the
/// ~250 Hz / ~4 kHz band edges. They only need to be *distinct* regions for the
/// differential columns to mean anything; nothing here is a contract with the
/// DSP's exact edges.
const BASS_BANDS: std::ops::Range<usize> = 0..22;
const MID_BANDS: std::ops::Range<usize> = 22..48;
const TREB_BANDS: std::ops::Range<usize> = 48..64;

/// A held frame with one scalar band up to `level` and the matching slice of the
/// log spectrum lit to the same level.
///
/// The slice tracks the scalar rather than carrying a second measured number:
/// the two are on different scales (the band array's own realistic level is
/// nothing like its scalar's — see [`LOW_LEVELS`]), and what a differential
/// column reads is a *ratio between two runs of the same shape*, so keeping one
/// level argument keeps the pair comparable.
fn band_stimulus(
    scalar: impl Fn(&mut AnalysisFrame, f32, f32),
    bands: std::ops::Range<usize>,
    level: f32,
    raw: f32,
) -> AnalysisFrame {
    let mut frame = AnalysisFrame::default();
    scalar(&mut frame, level, raw);
    for band in frame.spectrum.iter_mut().take(bands.end).skip(bands.start) {
        *band = level;
    }
    frame
}

/// The raw magnitude each normalized `1.0` corresponds to on the report's own
/// clip — the measured peaks, since `normalized = raw / peak` (ADR-0049).
///
/// A stimulus has to set the `*_raw` twin beside every normalized level it sets.
/// Leaving them at zero would report every `*_raw` gate in the library as dead,
/// which is precisely the "the report describes a preset that does not exist"
/// failure Plans 0041 and 0042 were spent closing.
const RAW_AT_FULL_SCALE: StimulusLevels = [0.106, 0.019, 0.032, 0.017];

/// The four stimuli at **full scale** — the historical reading. Every
/// `--report` number quoted in a commit message, an ADR Outcome or a backlog
/// entry was measured under these, and they are unchanged so those numbers keep
/// meaning what they said (ADR-0042).
fn band_stimuli() -> [AnalysisFrame; 4] {
    band_stimuli_at(FULL_LEVELS)
}

/// The same four stimuli at **realistic** levels ([`LOW_LEVELS`]). Read beside
/// the full-scale columns: the *gap* is the signal (ADR-0042). A binding gated
/// on a threshold real audio never reaches moves in one and not the other, and a
/// level `curve` — `level^curve`, the identity at `level = 1` for any exponent —
/// can only show up here.
fn band_stimuli_low() -> [AnalysisFrame; 4] {
    band_stimuli_at(LOW_LEVELS)
}

/// `[bass, mid, treb, onset]` levels for one reading.
type StimulusLevels = [f32; 4];

/// Full scale: every band pinned at `1.0`, the convention `--report` has always
/// used.
///
/// Since ADR-0049 this is also a level real audio *reaches* — all four normalized
/// signals touch `1.000` on the report's own clip — so the full-scale column
/// describes peaks rather than a fiction. Historical `--report` numbers were
/// measured under these same levels, but note that what `bass = 1.0` **means**
/// changed underneath them: pre-v2 it was an unreachable magnitude, now it is
/// "at its recent peak".
const FULL_LEVELS: StimulusLevels = [1.0, 1.0, 1.0, 1.0];

/// What the analyzer actually derives from music-like material, and therefore
/// what a shipped preset's thresholds and gains have to be written against.
///
/// **Re-measured 2026-07-30 for ADR-0049**, from `shot --signal dynamic:110`
/// over the [`REACH_SECS`] the reachability probe evaluates — so these levels and
/// the flags describe the same hops. Means, past warm-up:
///
/// ```text
/// variable    min     mean      max
/// bass      0.035    0.661    1.000
/// mid       0.031    0.575    1.000
/// treb      0.002    0.281    1.000
/// onset     0.001    0.145    1.000
/// ```
///
/// **These are normalized fractions of each signal's own recent peak, not
/// magnitudes** (ADR-0049). The previous values — `0.040 / 0.006 / 0.006 /
/// 0.0016` — were raw, and they are why the shipped library was gained 6-100x
/// hot: an author writing `bass > 0.3` was writing a gate real audio could not
/// reach. The `*_raw` variables still read on the old scale, and a threshold on
/// one of *those* still needs a table like the old one.
///
/// Two things follow, and both are improvements.
///
/// **Full scale stopped being fictional.** Every one of the four reaches `1.000`
/// on this clip, so [`FULL_LEVELS`] now describes a state real music produces on
/// every peak rather than a corner no signal ever visited. The gap between the
/// two columns therefore reads as *peak versus typical* — which is what an author
/// wants to know — instead of *reachable versus not*.
///
/// **The band array is still on its own scale**, for a new reason. It normalizes
/// against one peak shared by all 64 bands, so a single band only reaches `1.000`
/// when it *is* the loudest; across these hops the per-band mean is `0.089`
/// against the hottest band's `1.000`. So a `bin()`-reading preset still reads low
/// here relative to the scalars, and by more than the scalar gap suggests
/// (`docs/capturing.md`).
///
/// **Re-measure rather than guess** if you doubt them: `cargo run -p standalone
/// --example shot -- --signal dynamic:110 --out strip.png` prints the band table
/// on every run. These are one generator at one BPM — a judgment baked into the
/// harness (ADR-0042), which is why its provenance is written down instead of
/// the number standing alone.
const LOW_LEVELS: StimulusLevels = [0.661, 0.575, 0.281, 0.145];

fn band_stimuli_at(levels: StimulusLevels) -> [AnalysisFrame; 4] {
    let [bass, mid, treb, onset] = levels;
    let [bass_r, mid_r, treb_r, onset_r] = std::array::from_fn(|i| {
        levels.get(i).copied().unwrap_or(0.0) * RAW_AT_FULL_SCALE.get(i).copied().unwrap_or(0.0)
    });
    [
        band_stimulus(
            |f, v, r| {
                f.bass = v;
                f.bass_raw = r;
            },
            BASS_BANDS,
            bass,
            bass_r,
        ),
        band_stimulus(
            |f, v, r| {
                f.mid = v;
                f.mid_raw = r;
            },
            MID_BANDS,
            mid,
            mid_r,
        ),
        band_stimulus(
            |f, v, r| {
                f.treb = v;
                f.treb_raw = r;
            },
            TREB_BANDS,
            treb,
            treb_r,
        ),
        AnalysisFrame {
            // A transient is broadband, so the onset stimulus lights the whole
            // array rather than a slice.
            onset,
            onset_raw: onset_r,
            // `beat` stays true at both levels on purpose: it is an event, not a
            // magnitude, and real material does raise it. So a beat-latched
            // binding holds across the pair while an `onset`-scaled one falls
            // away — which is the distinction this column can now draw.
            beat: true,
            spectrum: [onset; SPECTRUM_BINS],
            ..Default::default()
        },
    ]
}

// ---------------------------------------------------------------------------
// Reachability (Plan 0041 Phase 3 / ADR-0042)
// ---------------------------------------------------------------------------

/// BPM the reachability probe generates. Matches the clip [`LOW_LEVELS`] was
/// measured from, so the columns and the flags describe the same material.
const REACH_BPM: f32 = 110.0;

/// Format the reachability probe synthesizes and analyzes at. Named once so the
/// hop clock `probe_reachability` derives `time` from cannot drift from the rate
/// the frames were actually produced at.
const REACH_FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// Seconds of that clip to evaluate over — deliberately longer than the 4 s a
/// `--signal` filmstrip synthesizes. The tempo tracker needs time to lock, and
/// under a 4 s clip it never does: `tempo` reads a flat `0`, which turns every
/// `tempo` comparison into a gate that was never really asked. At this length it
/// settles near the generator's own BPM, so a `tempo > 124` flag means "110 is
/// not above 124" — a true statement about one BPM — instead of "the tracker was
/// still cold".
const REACH_SECS: f32 = 12.0;

/// Positions the per-element `index` is sampled at for a binding that reads it.
/// Endpoints included, so a gate on the first or last element is exercised.
const REACH_INDEX_SAMPLES: usize = 5;

/// Analysis frames the reachability probe drives every expression with: the
/// **real** analyzer over the same generator [`LOW_LEVELS`] came from, not a
/// hand-built frame. Bands move together the way music's do, `bin()` sees a real
/// band array, and `beat`/`bar`/`onset` arrive as the detector produces them.
///
/// CPU only — no GPU, no rendering. This is the structural measurement
/// ADR-0042 adds beside the frame differentials.
fn reachability_frames() -> Result<Vec<AnalysisFrame>, String> {
    let format = REACH_FORMAT;
    let pcm = lmv_core::signal::dynamic_groove(REACH_BPM, REACH_SECS, format);
    let mut analyzer = Analyzer::new(format).map_err(|e| format!("reachability analyzer: {e}"))?;
    let hop_samples = HOP_SIZE * format.channels as usize;
    let mut frames = Vec::new();
    for (index, hop) in pcm.chunks(hop_samples).enumerate() {
        analyzer.push_interleaved(hop);
        let frame = analyzer.take_frame();
        // Skip warm-up for the same reason `band_levels` does: until the window
        // fills every band reads zero, and a gate would look dead on evidence
        // that is only the analyzer starting up.
        if index >= FILMSTRIP_WARMUP {
            frames.push(frame);
        }
    }
    if frames.is_empty() {
        return Err("reachability probe produced no analysis frames".to_string());
    }
    Ok(frames)
}

/// Walk every binding of `preset` under `frames`, and report the gates that
/// never went both ways.
///
/// **Every binding means the `[layer]`'s too** (Plan 0076 Phase 4): the layer's
/// params and its bindable `mix` are the same `Binding` machinery as the top
/// level — same compiled `Expr`, same `Observations` — so the walk is the same
/// loop over a longer list, and a layer gate that never fires flags exactly
/// like a top-level one. Layer entries are labeled `[layer] <param>` so the
/// report says which namespace a dead gate lives in.
fn probe_reachability(preset: &Preset, frames: &[AnalysisFrame]) -> Vec<GateReport> {
    let hop_seconds = HOP_SIZE as f32 / REACH_FORMAT.sample_rate as f32;
    let layer_bindings = preset
        .layer
        .iter()
        .flat_map(|layer| layer.params.iter().chain(layer.mix.iter()));
    let bindings = preset
        .params
        .iter()
        .map(|binding| (binding, false))
        .chain(layer_bindings.map(|binding| (binding, true)));
    let mut out = Vec::new();
    for (binding, in_layer) in bindings {
        let mut obs = Observations::new();
        for (hop, frame) in frames.iter().enumerate() {
            // Through the engine's own frame binding, so the probe cannot read
            // the frame differently than the renderer does — see
            // `Variables::from_frame`. Only `time` is ours to supply: there is
            // no render clock here, so it is the hop's position in the clip.
            //
            // Salted with the **pinned** salt, because the report is a capture
            // path (ADR-0051): it must describe the preset the harness renders,
            // and a `seed = "random"` preset's live salt is not that one.
            let vars = Variables::from_frame(frame, hop as f32 * hop_seconds)
                .with_salt(preset.pinned_salt);
            // A per-element binding is evaluated once per element by the render
            // loop, so a gate of its can be live at one end of the strip and
            // dead at the other. Sampling `index` is what keeps this honest.
            if binding.expr.uses_index() {
                for step in 0..REACH_INDEX_SAMPLES {
                    let t = step as f32 / (REACH_INDEX_SAMPLES.max(2) - 1) as f32;
                    binding.expr.eval_probed(&vars.with_index(t), &mut obs);
                }
            } else {
                binding.expr.eval_probed(&vars, &mut obs);
            }
        }
        for flag in binding.expr.flag_gates(&obs) {
            out.push(GateReport {
                param: if in_layer {
                    format!("[layer] {}", binding.name)
                } else {
                    binding.name.clone()
                },
                flag,
            });
        }
    }
    out
}

/// `(preset name, flagged gates)` for the whole library. Run **before** the
/// renderer takes ownership of the presets — it reads their compiled
/// expressions, which is the one thing a capture cannot show.
fn reachability_pass(presets: &[Preset]) -> Result<Vec<(String, Vec<GateReport>)>, String> {
    let frames = reachability_frames()?;
    Ok(presets
        .iter()
        .map(|p| (p.name.clone(), probe_reachability(p, &frames)))
        .collect())
}

/// Ceilings named in the text report per family. The whole library trips over
/// 200 of these — nearly every `clamp()` in it was written as a ceiling for
/// full-scale input, which is the same mis-gaining the columns show — and
/// printing them all buries the dozen dead branches that are actually
/// actionable. The count is per preset in the table, the worst few are named
/// here, and `--json` carries every one.
const CEILINGS_NAMED: usize = 3;

/// The per-family saturation block: every `clamp()` that spent the run pinned
/// at its upper bound, named one per line.
///
/// Named individually rather than summarized the way ceilings are, and the
/// asymmetry is the point (ADR-0062). An unapproached ceiling is a shrug — the
/// library trips over two hundred of them and every one only narrows a
/// parameter's real range. A saturated one is a binding that has stopped
/// reading the audio at all, it is a HARD failure in
/// `core/tests/saturation.rs`, and there should be **none** here. A list that is
/// ever long is a library-wide event, not a table to skim.
fn write_saturation(out: &mut String, fam: &FamilyReport) {
    let saturated: Vec<(&str, &GateReport)> = fam
        .presets
        .iter()
        .flat_map(|p| {
            p.gates
                .iter()
                .filter(|g| is_saturated(g))
                .map(move |g| (p.name.as_str(), g))
        })
        .collect();
    if saturated.is_empty() {
        let _ = writeln!(
            out,
            "  no clamp sat at its upper bound past {:.0}% of the probe",
            SATURATED_OCCUPANCY * 100.0
        );
        return;
    }
    let _ = writeln!(
        out,
        "  a clamp pinned at its bound is a gain, not a limit: divide it until the \
         bound is reached only on peaks. `core/tests/saturation.rs` fails on these \
         unless the preset declares an [occupancy] exemption"
    );
    for (name, gate) in &saturated {
        let _ = writeln!(out, "{}", gate_line(name, gate));
    }
}

/// The per-family ceiling line: how many `clamp()` upper bounds never bit, and
/// the ones furthest from biting.
fn write_ceiling_summary(out: &mut String, fam: &FamilyReport) {
    let mut ceilings: Vec<(&str, &GateReport, f32)> = fam
        .presets
        .iter()
        .flat_map(|p| {
            p.gates.iter().filter_map(move |g| match g.flag.kind {
                GateKind::Clamp {
                    peak_fraction_of_bound,
                } => Some((p.name.as_str(), g, peak_fraction_of_bound)),
                GateKind::Select { .. } | GateKind::Compare { .. } | GateKind::Saturated { .. } => {
                    None
                }
            })
        })
        .collect();
    if ceilings.is_empty() {
        return;
    }
    // Furthest from its bound first — the most decorative ceiling is the one
    // most worth re-gaining.
    ceilings.sort_by(|a, b| a.2.total_cmp(&b.2));
    let named: Vec<String> = ceilings
        .iter()
        .take(CEILINGS_NAMED)
        .map(|(name, gate, fraction)| format!("{name}.{} at {:.0}%", gate.param, fraction * 100.0))
        .collect();
    let _ = writeln!(
        out,
        "  {} clamp ceiling(s) never approached at this level (furthest: {}) — \
         a bound that never bites is a parameter with a narrower real range than \
         it reads; --json lists them all",
        ceilings.len(),
        named.join(", ")
    );
}

/// One flagged gate as a line of report text. Worded as a suspect: it says what
/// was observed, not what is wrong.
fn gate_line(preset: &str, gate: &GateReport) -> String {
    match gate.flag.kind {
        GateKind::Select { always } => {
            let (never, branch) = if always {
                ("false", "else")
            } else {
                ("true", "then")
            };
            format!(
                "  GATE {preset}.{}: `{}` never went {never}, so its `{branch}` branch never ran",
                gate.param, gate.flag.source
            )
        }
        // A comparison has no branches to name, so the consequence is stated as
        // the constant it collapsed to — which is what an author has to picture
        // to see that a boolean param never turned on (ADR-0043).
        GateKind::Compare { always } => {
            let (never, constant) = if always { ("false", 1) } else { ("true", 0) };
            format!(
                "  COMP {preset}.{}: `{}` never went {never}, so it read as a constant {constant}",
                gate.param, gate.flag.source
            )
        }
        GateKind::Clamp {
            peak_fraction_of_bound,
        } => format!(
            "  CEIL {preset}.{}: `{}` reached {:.0}% of its upper bound",
            gate.param,
            gate.flag.source,
            peak_fraction_of_bound * 100.0
        ),
        GateKind::Saturated { occupancy } => format!(
            "  SAT  {preset}.{}: `{}` sat at its upper bound on {:.0}% of hops",
            gate.param,
            gate.flag.source,
            occupancy * 100.0
        ),
    }
}

/// The probe stimulus (Plan 0037): silence, a step up to [`loud_frame`], then a
/// step back down. This is the whole reason the probe can see easing at all — a
/// held stimulus converges every smoother before the pixels are read, so it
/// reports the settled response and is identical for any `[smoothing]` constant.
///
/// It steps to `loud_frame()` rather than to a hand-built frame precisely so the
/// log-band array is lit on the same convention every other stimulus here uses.
fn step_stimulus() -> Vec<AnalysisFrame> {
    let silent = AnalysisFrame::default();
    let mut frames = vec![silent; PROBE_PRE];
    frames.extend(std::iter::repeat_n(loud_frame(), PROBE_WINDOW));
    frames.extend(std::iter::repeat_n(silent, PROBE_WINDOW));
    frames
}

/// A transient measurement together with whether each direction actually
/// arrived anywhere inside its window (Plan 0038 Phase 8).
///
/// The two booleans are not decoration. `frames_to_settle` normalizes against
/// its segment's own last frame, so it *always* answers inside the segment and
/// its output carries no evidence about its own validity — a truncated response
/// and a settled one are indistinguishable from the number alone. Only
/// `segment_settled` can tell them apart, which is why an unmarked cell is a
/// claim this report was not previously entitled to make.
#[derive(Clone, Copy)]
struct Transient {
    response: StepResponse,
    rise_settled: bool,
    fall_settled: bool,
}

/// Measure a probe capture. Each segment starts at the last frame *before* its
/// step, which is the settled state the response departs from.
fn probe_response(images: &[CaptureImage]) -> Transient {
    let rise = images
        .get(PROBE_PRE - 1..PROBE_PRE + PROBE_WINDOW)
        .unwrap_or_default();
    let fall = images
        .get(PROBE_PRE + PROBE_WINDOW - 1..PROBE_PRE + 2 * PROBE_WINDOW)
        .unwrap_or_default();
    Transient {
        response: step_response(rise, fall),
        rise_settled: segment_settled(rise, PROBE_SETTLE_TOL),
        fall_settled: segment_settled(fall, PROBE_SETTLE_TOL),
    }
}

/// One transient cell: the frame count, suffixed `+` when `segment_settled`
/// could not certify the response arrived. Read a marked cell as *at least this
/// many*, never as a measurement.
///
/// **Most of the shipped set marks, for two different reasons the suffix cannot
/// tell apart** (measured, Plan 0038 Phase 8). Either the response outran the
/// 0.8 s window — a release above ~0.2 s does — or, far more commonly here, the
/// scene never stops moving, so there is no asymptote to settle to and
/// `segment_settled` correctly declines to certify one. The second is the same
/// limitation `docs/capturing.md` already documents as "the scene's own motion is
/// measured too"; the mark makes it visible per cell instead of leaving it as a
/// caveat the reader has to remember.
fn transient_cell(frames: u32, settled: bool) -> String {
    if settled {
        frames.to_string()
    } else {
        format!("{frames}+")
    }
}

/// `(dead gates, unapproached ceilings, saturated clamps)` for one preset.
/// Counted apart because they are three different claims: a one-sided
/// `select()` or comparison means a branch of the preset has never rendered, a
/// `clamp()` short of its bound only means the ceiling is doing no work, and a
/// `clamp()` **at** its bound throughout means the binding stopped being a
/// function of the audio (ADR-0062).
///
/// Every count is by kind rather than by subtraction: "everything that is not a
/// dead gate" was already a ceiling count only by coincidence, and adding a
/// third kind would have made it report saturation as its exact opposite.
fn gate_counts(p: &PresetReport) -> (usize, usize, usize) {
    let count = |f: fn(&GateReport) -> bool| p.gates.iter().filter(|g| f(g)).count();
    (
        count(is_dead_gate),
        count(|g| matches!(g.flag.kind, GateKind::Clamp { .. })),
        count(is_saturated),
    )
}

/// Whether this flag is a never-exercised gate rather than a decorative
/// ceiling. A comparison counts with the `select()`s: both say a branch of the
/// preset's behavior has never happened (ADR-0043).
fn is_dead_gate(gate: &GateReport) -> bool {
    matches!(
        gate.flag.kind,
        GateKind::Select { .. } | GateKind::Compare { .. }
    )
}

/// Whether this flag is a `clamp()` that spent the run pinned at its upper
/// bound (ADR-0062) — the finding `core/tests/saturation.rs` gates on.
fn is_saturated(gate: &GateReport) -> bool {
    matches!(gate.flag.kind, GateKind::Saturated { .. })
}

fn loud_frame() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        // "Every band up" includes the log-band array itself — see
        // [`BASS_BANDS`].
        spectrum: [1.0; SPECTRUM_BINS],
        ..Default::default()
    }
}

fn corner(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        255,
    ]
}

/// The --report text tables, as a string.
///
/// Returns rather than prints so the formatting is directly testable - that is
/// the whole reason this module moved into the library (Plan 0061 Phase 4).
fn text_report(source: &str, reports: &[FamilyReport], tier: Tier) -> String {
    let mut out = String::new();
    // The tier is in the header because every number below is measured at it: a
    // report read against a preset rendered on another tier is comparing two
    // different capacity budgets (ADR-0045).
    let _ = writeln!(out, "visual-QA report [{source}] tier {}", tier.as_str());
    for fam in reports {
        let _ = writeln!(
            out,
            "\n=== {} ({} presets) ===",
            fam.system.as_str(),
            fam.presets.len()
        );
        // The `geom` column exists only where the measurement does: a family
        // whose scenes build no segment list has no line seam to measure at,
        // and a fabricated cell would read as a finding (backlog 0070).
        let show_geometry = fam.presets.iter().any(|p| p.geometry.is_some());
        let mut header = format!(
            "  {:<14} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>5} {:>5}",
            "preset", "bass", "mid", "treb", "onset", "anim", "cover", "rise", "fall"
        );
        if show_geometry {
            let _ = write!(header, " {:>6}", "geom");
        }
        let _ = writeln!(out, "{header}");
        for p in &fam.presets {
            let mut row = format!(
                "  {:<14.14} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>5} {:>5}",
                p.name,
                p.reactivity[0],
                p.reactivity[1],
                p.reactivity[2],
                p.reactivity[3],
                p.animation,
                p.coverage,
                transient_cell(p.transient.response.rise_frames, p.transient.rise_settled),
                transient_cell(p.transient.response.fall_frames, p.transient.fall_settled),
            );
            if show_geometry {
                match p.geometry {
                    Some(fraction) => {
                        let _ = write!(row, " {fraction:>6.4}");
                    }
                    None => {
                        let _ = write!(row, " {:>6}", "-");
                    }
                }
            }
            let _ = writeln!(out, "{row}");
        }
        if show_geometry {
            let _ = writeln!(
                out,
                "  geom is the in-frame geometry fraction: the share of drawn segment \
                 length inside the frame, at the fully-driven capture (ADR-0083). Read \
                 it while tuning `scale` — no absolute threshold orders the library, \
                 and two shipped presets deliberately leave the frame"
            );
        }
        // The second reading, as its own block rather than four more columns:
        // the table above is already nine wide and a wide terminal is not
        // guaranteed (ADR-0042). Keeping it un-widened also means every number
        // a previous run printed is still in the same place.
        let [bass_lo, mid_lo, treb_lo, onset_lo] = LOW_LEVELS;
        let _ = writeln!(
            out,
            "\n  at realistic levels (bass {bass_lo} mid {mid_lo} treb {treb_lo} onset \
             {onset_lo}) — read the *gap* against the columns above, not the value alone:"
        );
        let _ = writeln!(
            out,
            "  {:<14} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>5}",
            "preset", "bass", "mid", "treb", "onset", "gates", "ceils", "occ"
        );
        for p in &fam.presets {
            let (dead, ceilings, saturated) = gate_counts(p);
            let _ = writeln!(
                out,
                "  {:<14.14} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7} {:>7} {:>5}",
                p.name,
                p.reactivity_low[0],
                p.reactivity_low[1],
                p.reactivity_low[2],
                p.reactivity_low[3],
                dead,
                ceilings,
                saturated,
            );
        }
        let dead: Vec<(&str, &GateReport)> = fam
            .presets
            .iter()
            .flat_map(|p| {
                p.gates
                    .iter()
                    .filter(|g| is_dead_gate(g))
                    .map(move |g| (p.name.as_str(), g))
            })
            .collect();
        if dead.is_empty() {
            let _ = writeln!(
                out,
                "  every branch was taken under the {REACH_BPM} BPM probe"
            );
        } else {
            let _ = writeln!(
                out,
                "  a flag is a suspect, not a conviction: it says this {REACH_SECS} s \
                 {REACH_BPM} BPM probe never drove the gate both ways. A gate on `tempo` \
                 is correctly one-sided under a single BPM"
            );
            for (name, gate) in &dead {
                let _ = writeln!(out, "{}", gate_line(name, gate));
            }
        }
        write_saturation(&mut out, fam);
        write_ceiling_summary(&mut out, fam);

        let marked = fam
            .presets
            .iter()
            .filter(|p| !p.transient.rise_settled || !p.transient.fall_settled)
            .count();
        if marked > 0 {
            let _ = writeln!(
                out,
                "  {marked} of {} presets carry a `+` transient cell: not a settled \
                 measurement, so read it as a lower bound. Two causes, and the mark \
                 does not distinguish them — the response outran the \
                 {PROBE_WINDOW}-frame window, or the scene's own motion means it has \
                 no asymptote to settle to at all",
                fam.presets.len()
            );
        }
        if fam.near_dups.is_empty() {
            let _ = writeln!(
                out,
                "  near-duplicate geometry: none below shape {NEAR_DUP_STRUCT}"
            );
        } else {
            for (a, b) in &fam.near_dups {
                let _ = writeln!(out, "  NEAR-DUP: {a} ~ {b}");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Hand-rolled JSON (fixed numeric schema, no serde)
// ---------------------------------------------------------------------------

fn render_json(source: &str, reports: &[FamilyReport], tier: Tier) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str(&format!("\"source\":{},", json_string(source)));
    out.push_str(&format!("\"tier\":{},", json_string(tier.as_str())));
    out.push_str("\"families\":{");
    for (fi, fam) in reports.iter().enumerate() {
        if fi > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}:{{", json_string(fam.system.as_str())));
        // presets
        out.push_str("\"presets\":{");
        for (pi, p) in fam.presets.iter().enumerate() {
            if pi > 0 {
                out.push(',');
            }
            out.push_str(&format!("{}:{{", json_string(&p.name)));
            out.push_str(&format!(
                "\"reactivity\":{{\"bass\":{},\"mid\":{},\"treb\":{},\"onset\":{}}},",
                num(p.reactivity[0]),
                num(p.reactivity[1]),
                num(p.reactivity[2]),
                num(p.reactivity[3]),
            ));
            out.push_str(&format!(
                "\"reactivity_low\":{{\"bass\":{},\"mid\":{},\"treb\":{},\"onset\":{}}},",
                num(p.reactivity_low[0]),
                num(p.reactivity_low[1]),
                num(p.reactivity_low[2]),
                num(p.reactivity_low[3]),
            ));
            out.push_str(&format!("\"animation\":{},", num(p.animation)));
            out.push_str(&format!("\"coverage\":{},", num(p.coverage)));
            // Only where the measurement exists — the JSON mirrors the text
            // table's omission rather than inventing a null convention.
            if let Some(fraction) = p.geometry {
                out.push_str(&format!("\"in_frame_geometry\":{},", num(fraction)));
            }
            out.push_str(&format!(
                "\"transient\":{},",
                json_transient(
                    p.transient.response.rise_frames,
                    p.transient.response.fall_frames,
                    p.transient.response.ratio(),
                    p.transient.rise_settled,
                    p.transient.fall_settled,
                )
            ));
            out.push_str(&format!("\"reachability\":{}", json_reachability(p)));
            out.push('}');
        }
        out.push_str("},");
        // distinctness
        out.push_str("\"distinctness\":{");
        out.push_str(&format!("\"pixel\":{},", json_matrix(&fam.pixel)));
        out.push_str(&format!("\"shape\":{},", json_matrix(&fam.shape)));
        out.push_str("\"near_duplicates\":[");
        for (di, (a, b)) in fam.near_dups.iter().enumerate() {
            if di > 0 {
                out.push(',');
            }
            out.push_str(&format!("[{},{}]", json_string(a), json_string(b)));
        }
        out.push(']');
        out.push('}');
        out.push('}');
    }
    out.push_str("}}");
    out.push('\n');
    out
}

/// One preset's reachability findings, machine-readable: the same counts the
/// table shows plus every flagged gate named by parameter and source text.
///
/// `probe` records what produced them, because a flag only ever means "not
/// observed under *this* stimulus" — a consumer that drops the provenance is
/// reading the numbers as convictions.
fn json_reachability(p: &PresetReport) -> String {
    let (dead, ceilings, saturated) = gate_counts(p);
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"probe\":{{\"signal\":\"dynamic\",\"bpm\":{},\"seconds\":{}}},\
         \"dead_branches\":{dead},\"unapproached_ceilings\":{ceilings},\
         \"saturated_clamps\":{saturated},\"gates\":[",
        num(REACH_BPM),
        num(REACH_SECS),
    ));
    for (i, gate) in p.gates.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"param\":{},\"source\":{},",
            json_string(&gate.param),
            json_string(&gate.flag.source)
        ));
        match gate.flag.kind {
            GateKind::Select { always } => out.push_str(&format!(
                "\"kind\":\"select\",\"always\":{always}}}",
                always = if always { "true" } else { "false" }
            )),
            GateKind::Compare { always } => out.push_str(&format!(
                "\"kind\":\"compare\",\"always\":{always}}}",
                always = if always { "true" } else { "false" }
            )),
            GateKind::Clamp {
                peak_fraction_of_bound,
            } => out.push_str(&format!(
                "\"kind\":\"clamp\",\"peak_fraction_of_bound\":{}}}",
                num(peak_fraction_of_bound)
            )),
            GateKind::Saturated { occupancy } => out.push_str(&format!(
                "\"kind\":\"saturated\",\"occupancy\":{}}}",
                num(occupancy)
            )),
        }
    }
    out.push_str("]}");
    out
}

#[cfg(test)]
mod tests;
