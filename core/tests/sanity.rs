//! Shape sanity (Plan 0013 Phase 3, HARD). A newly-added scene that drew nothing
//! or a single dot should fail before it ships. Under a sustained *loud* frame
//! (so audio-gated brightness is up), assert each preset lights a minimum
//! fraction of the frame (`coverage`) and spreads across at least two quadrants
//! (`quadrant_spread`) — "not blank, not a dot".
//!
//! **Plan 0058 / [ADR-0067]: the capture measures the scene, not the backdrop.**
//! This gate used to sample the background from pixel (0, 0) — the frame's own
//! corner — on the Plan 0013 reasoning that a scene which clears to a dark blue
//! would otherwise score as fully lit. That reasoning was correct for a
//! per-scene clear and became wrong the day the backdrop moved into an engine
//! pre-pass ([ADR-0018](../../docs/adrs/0018-background-pre-pass.md)):
//! `bg_vignette` darkens the frame toward its edges, so on any preset that binds
//! one **the corner is the darkest pixel in the image** and nearly every pixel
//! toward the centre differs from it by more than [`EPS`]. The backdrop read as
//! a large, well-spread, lit figure. 24 of the 35 shipped presets bind
//! `bg_vignette`, and the sparse-system floor is 0.01, so for most of the
//! library the floor was satisfied by the backdrop alone whatever the scene did
//! — an unfalsifiable gate, which `spectrum_ridge` proved by shipping a contour
//! drawn 3.3 world units off the top of a frame of half-height 1.0 and passing.
//!
//! So the roster this gate renders has its `bg_*` bindings **removed**
//! ([`without_backdrop`]) and `is_lit` compares against [`BLACK`]. The
//! background stage already defaults `bright` and `vignette` to `0.0`
//! (`core/src/render/background.rs`), so this is *not applying three bindings*
//! rather than a new render path: the pass renders the plain black clear it
//! renders for any preset that never mentions `bg_*`. Nothing outside this file
//! changes — `golden`, `distinctness`, `reactivity` and `shot` all keep the
//! shipped composite, backdrop included.
//!
//! Coverage floors are per-system: `fragment_field` fills the frame, while the
//! `swarm` is sparse points, so a single broad floor would be either tautological
//! for one or impossible for the other.
//!
//! **Plan 0056 Phase 5 adds a third question: does the shape have an interior?**
//! "Not blank, not a dot" is satisfied completely by a fully saturated
//! single-tone mass — a real figure, the right size, in every quadrant, and a
//! blot. That is how four attractor presets shipped flat behind this gate, and
//! `tonal_flatness` is the statistic that names it. It is general, not
//! attractor-specific: any drive that stacks past the additive ceiling produces
//! it.

use lmv_core::{
    dsp::AnalysisFrame,
    preset::{Preset, SystemKind, default_presets},
    render::{
        HeadlessOptions, RenderError, Renderer,
        metrics::{TONE_BANDS, coverage, quadrant_spread, tonal_flatness},
    },
};

const SIZE: u32 = 96;
const FRAMES: u32 = 30;
/// A pixel counts as lit if any RGB channel differs from [`BLACK`] by more than
/// this (shrugs off dark near-black dithering).
const EPS: u8 = 10;
/// What the scene is measured against (ADR-0067). Not a sampled pixel: the
/// backdrop is suppressed for this capture, so every lit pixel is light the
/// **scene** put there. Alpha is never compared — [`is_lit`] takes the first
/// three channels — but the frames come back opaque, so 255 is the honest value.
///
/// [`is_lit`]: lmv_core::render::metrics
const BLACK: [u8; 4] = [0, 0, 0, 255];
/// The prefix every background-stage parameter carries (`bg_hue`, `bg_bright`,
/// `bg_vignette` — `core/src/render/background.rs`'s `PARAMS`, which is
/// `pub(crate)` and so not nameable from an integration test).
/// [`sanity_roster`] asserts the prefix still matches something, so a rename
/// fails this gate rather than silently restoring the backdrop.
const BG_PREFIX: &str = "bg_";
/// Minimum lit quadrants — a dot in one corner fails.
const MIN_QUADRANTS: u8 = 2;

/// Maximum share of the lit figure that may sit inside one narrow luminance
/// band (Plan 0056 Phase 5, backlog 0047) — the point past which the picture has
/// no tonal structure left, only a mass of one tone.
///
/// `coverage` and `quadrant_spread` ask *is something there* and *is it more
/// than a dot*, and a fully saturated single-tone mass answers yes to both: it
/// is a real shape, the right size, in every quadrant, and it is also a blot.
/// This is the third question.
///
/// **Measured, from the shipped library's own values.**
/// `every_preset_draws_a_real_shape` prints the whole distribution on every run.
/// Re-measured after `Spectrum Ridge` was repaired and left [`KNOWN_FLAT`]:
///
/// ```text
/// 0.8655  Spectrum Ridge      0.6438  De Jong
/// 0.8300  Rose Trails         0.4923  Coral Head
/// 0.7645  Rose Web            0.4518  Coral Bloom
/// 0.6588  Coral               0.4453  Leviathan
/// ```
///
/// Everything else is below `0.45`. The deliberately flattened fixture reads
/// `0.98`, so `0.90` still sits between the library and the fixture — but the
/// margin above is now **`0.035`**, not the comfortable gap this constant was
/// first set with, and the top three are structural rather than accidental: a
/// polyline of near-equal values *is* a straight line, and a trails-heavy line
/// look is mostly faint tail at one level. Under [`loud`] every band is driven to
/// `1.0` at once, which is the worst possible stimulus for exactly those shapes.
///
/// So do not read a pass here as headroom. A measured constant with a shelf
/// life: re-measure when the library changes materially, and if the top of the
/// distribution keeps climbing, the thing to question is whether a flat-spectrum
/// stimulus can fairly judge a spectrum readout — not whether to nudge `0.90`.
const MAX_TONAL_FLATNESS: f32 = 0.90;

/// Shipped presets that are flat **today**, tracked rather than gated.
///
/// A defect list, not a policy — and it is **empty**, which is the state to keep
/// it in. An entry here is asserted to *still* be flat below, so a repaired
/// preset fails this test and tells you to delete its line rather than leaving a
/// stale exemption behind.
///
/// Its one entry, `Spectrum Ridge`, was carried from Plan 0056 (which was
/// test-and-harness only and so could not repair content) and removed when the
/// preset was fixed: `1.000` → `0.8655`. Worth knowing what that repair actually
/// was, because the list's original note had it wrong. The mechanism was not the
/// additive stacking the preset's header describes — it was that `scale = 3.20`,
/// tuned before ADR-0049 normalized the bands, put a driven element about 3.3
/// world units up against a visible half-height of `1.0`. The contour was **off
/// frame entirely**, and the `1.000` was the lit `bg_vignette` left behind, not
/// the preset. See [design-backlog 0053](../../docs/design-backlog.md): neither
/// `coverage` nor `quadrant_spread` can distinguish a vignette from a figure, so
/// this statistic convicted the right preset for the wrong reason.
const KNOWN_FLAT: &[&str] = &[];

/// Per-system minimum lit fraction. The full-screen field must fill most of the
/// frame; the sparse swarm need only paint a small but real footprint.
fn coverage_floor(system: SystemKind) -> f32 {
    match system {
        // Full-screen field fills most of the frame.
        SystemKind::FragmentField => 0.30,
        // Reaction-diffusion paints a real pattern across the frame, but the
        // present maps only the sparse V species, so the lit fraction is modest.
        SystemKind::ReactionDiffusion => 0.03,
        // Sparse line art / point swarm / attractor cloud / spectrum comb: a
        // small but real footprint.
        SystemKind::Swarm
        | SystemKind::ParametricCurve
        | SystemKind::LSystem
        | SystemKind::StarPattern
        | SystemKind::Attractor
        | SystemKind::Spectrum => 0.01,
    }
}

fn system_name(system: SystemKind) -> &'static str {
    match system {
        SystemKind::FragmentField => "fragment_field",
        SystemKind::Swarm => "swarm",
        SystemKind::ParametricCurve => "parametric_curve",
        SystemKind::LSystem => "lsystem",
        SystemKind::StarPattern => "star_pattern",
        SystemKind::ReactionDiffusion => "reaction_diffusion",
        SystemKind::Attractor => "attractor",
        SystemKind::Spectrum => "spectrum",
    }
}

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
/// Any other build error still panics loudly.
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// A sustained "loud" frame: every band up and a beat, so any audio-gated
/// brightness reaches its lit state.
///
/// "Every band up" now includes the `spectrum` array itself (Plan 0034 Phase 2).
/// A frame with `bass = mid = treb = 1.0` and 64 silent log-bands is not a frame
/// any audio could produce, and under it a spectrum readout would correctly draw
/// almost nothing — the floor would be measuring the fixture, not the scene. No
/// pre-0034 scene reads `spectrum`, so every other preset's capture is
/// unchanged.
fn loud() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; lmv_core::dsp::SPECTRUM_BINS],
        ..Default::default()
    }
}

/// Drop the preset's backdrop bindings so the capture renders the scene over the
/// background stage's default black (ADR-0067).
///
/// A **test-side** transform on purpose: the renderer's capture surface is not
/// widened, no engine flag is added, and every other caller keeps the shipped
/// composite. Removing the bindings is enough because the stage's own defaults
/// are `bright = 0.0` / `vignette = 0.0`, and at `bg_bright <= 0` the pass is a
/// plain black clear that does not even build its gradient pipeline.
fn without_backdrop(mut preset: Preset) -> Preset {
    preset.params.retain(|b| !b.name.starts_with(BG_PREFIX));
    preset
}

/// The shipped library with its backdrops suppressed, plus the `(name, system)`
/// of each preset in roster order.
///
/// Panics if the transform matched nothing. That is the guard on the guard: if
/// the background params are ever renamed off `bg_`, this file would quietly go
/// back to measuring vignettes and every floor below would go back to being
/// unfalsifiable, with a green suite the whole way.
fn sanity_roster() -> (Vec<Preset>, Vec<(String, SystemKind)>) {
    let mut stripped = 0usize;
    let mut with_backdrop = 0usize;
    let presets: Vec<Preset> = default_presets()
        .into_iter()
        .map(|p| {
            let before = p.params.len();
            let p = without_backdrop(p);
            let removed = before - p.params.len();
            stripped += removed;
            with_backdrop += usize::from(removed > 0);
            p
        })
        .collect();
    assert!(
        stripped > 0,
        "no `{BG_PREFIX}*` binding was found in any of the {} shipped presets — the \
         backdrop suppression this gate rests on (ADR-0067) has become a no-op, so \
         `coverage` is measuring the backdrop again",
        presets.len()
    );
    println!(
        "backdrop suppressed: {stripped} {BG_PREFIX}* binding(s) removed across \
         {with_backdrop}/{} presets",
        presets.len()
    );
    let meta = presets.iter().map(|p| (p.name.clone(), p.system)).collect();
    (presets, meta)
}

#[test]
fn every_preset_draws_a_real_shape() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = loud();
    let (presets, meta) = sanity_roster();
    renderer.set_presets(presets);

    let mut failures = Vec::new();
    let mut flatness = Vec::new();
    for (name, system) in &meta {
        let (name, system) = (name.as_str(), *system);
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture preset");
        let cov = coverage(&img, BLACK, EPS);
        let spread = quadrant_spread(&img, BLACK, EPS);
        let flat = tonal_flatness(&img, BLACK, EPS);
        let floor = coverage_floor(system);
        println!(
            "[{}] {name:<12} coverage={cov:.4} (floor {floor:.2}) quadrants={spread} \
             flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2})",
            system_name(system),
        );
        let known_flat = KNOWN_FLAT.contains(&name);
        flatness.push((flat, name.to_string(), known_flat));
        if cov < floor {
            failures.push(format!("{name} blank: coverage {cov:.4} < {floor:.2}"));
        }
        if spread < MIN_QUADRANTS {
            failures.push(format!(
                "{name} is a dot: {spread} quadrant(s) < {MIN_QUADRANTS}"
            ));
        }
        if flat > MAX_TONAL_FLATNESS && !known_flat {
            failures.push(format!(
                "{name} is flat: {:.1}% of its lit pixels sit in one of {TONE_BANDS} luminance \
                 bands (max {:.0}%) — a real shape with no interior, which coverage and \
                 spread both score as healthy. Lower the drive, the glow or the \
                 accumulation until the figure has falloff again",
                flat * 100.0,
                MAX_TONAL_FLATNESS * 100.0,
            ));
        }
        // The list must not outlive the defect. A repaired preset that is still
        // named here would silently exempt whatever it becomes next.
        if known_flat && flat <= MAX_TONAL_FLATNESS {
            failures.push(format!(
                "{name} is listed in KNOWN_FLAT but now measures {flat:.4}, under the \
                 {MAX_TONAL_FLATNESS:.2} ceiling — it was repaired, so delete the entry"
            ));
        }
    }

    // The distribution the threshold above is set from, printed on every run so
    // the next re-measurement does not need a special one.
    flatness.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("flattest presets (share of lit pixels in one luminance band):");
    for (flat, name, known) in flatness.iter().take(8) {
        let mark = if *known { "  (KNOWN_FLAT)" } else { "" };
        println!("  {flat:.4}  {name}{mark}");
    }

    assert!(
        failures.is_empty(),
        "these presets failed shape sanity: {failures:#?}"
    );
}

/// A line scene driven far past the additive ceiling: strokes wide enough to
/// meet, a glow multiplier that saturates every core, and a long trail that
/// stacks the same light again — so the whole figure clips to one tone.
///
/// Deliberately built the way the *shipped* flat frames got there (an additive
/// stack, not an `exposure` stop), because that is the failure mode this gate
/// exists to name. Exposure alone will not do it: past the knee the background
/// blows out with the figure, and a background-relative metric correctly stops
/// finding anything lit.
fn blown_out() -> Preset {
    Preset::from_toml_str(
        r#"
system = "parametric_curve"
name   = "Blown Out"

[params]
scale      = "0.9"
glow       = "20"
brightness = "16"
thickness  = "44"
trails     = "0.97"
"#,
    )
    .expect("the flat fixture parses")
}

#[test]
fn a_frame_with_no_tonal_structure_is_reported_flat() {
    let Some(mut renderer) = headless() else {
        return;
    };
    renderer.set_presets(vec![without_backdrop(blown_out())]);
    let img = renderer
        .capture_preset("Blown Out", &loud(), FRAMES)
        .expect("capture the flat fixture");

    let cov = coverage(&img, BLACK, EPS);
    let spread = quadrant_spread(&img, BLACK, EPS);
    let flat = tonal_flatness(&img, BLACK, EPS);
    println!("[blown out] coverage={cov:.4} quadrants={spread} flatness={flat:.4}");

    // The fixture has to pass the two existing checks, or it demonstrates
    // nothing: the whole claim is that a blot satisfies both of them.
    assert!(
        cov >= coverage_floor(SystemKind::ParametricCurve),
        "the fixture must pass the coverage floor, or it proves nothing: {cov:.4}"
    );
    assert!(
        spread >= MIN_QUADRANTS,
        "the fixture must pass the spread floor, or it proves nothing: {spread}"
    );
    assert!(
        flat > MAX_TONAL_FLATNESS,
        "a figure stacked past the additive ceiling must read flat, got {flat:.4}"
    );
}

/// **`spectrum_ridge` exactly as it shipped broken**, recovered from
/// `git show 81190ac^:presets/spectrum_ridge.toml` — every table and every
/// binding byte-for-byte, comments stripped and the `name` suffixed so the
/// output reads clearly. Nothing here is tunable: this is the defect, frozen.
///
/// `scale = 3.20` is the whole of it. Tuned before
/// [ADR-0049](../../docs/adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md)
/// normalized the bands to `0..1`, it afterwards multiplied a value roughly five
/// times larger, putting a driven element about **3.3 world units** up against a
/// visible half-height of `1.0`. Under [`loud`] the contour is off frame
/// entirely and the composite comes back empty except for `bg_vignette`.
fn pre_repair_spectrum_ridge() -> Preset {
    Preset::from_toml_str(
        r#"
system = "spectrum"
name   = "Spectrum Ridge (pre-repair)"

[spectrum]
elements = 40
layout   = "polyline"
smoothing = { attack = 0.04, release = 0.34 }

[palette]
name = "aurora"

[params]
base  = "0.12 + sin(time * 1.3) * 0.12"
scale = "3.20"
curve = "0.55"
span  = "1.72 + sin(time * 0.31) * 0.16"
mirror_order   = "1"
mirror_reflect = "1"
baseline       = "0"
rotation = "sin(time * 0.9) * 0.40 + clamp(bass * 0.118, 0, 0.10)"
hue        = "mod(0.10 + time * 0.02, 1)"
hue_spread = "0.75"
saturation = "0.95"
thickness  = "7.40 + clamp(mid * 3.06, 0, 2.6)"
glow       = "1.12 + clamp(bass * 0.235, 0, 0.20)"
brightness = "0.80 + clamp(bass * 0.212, 0, 0.18)"
zoom  = "1.00 + sin(time * 0.23) * 0.05"
pan_y = "0.02"
bg_hue      = "0.44 + sin(time * 0.008) * 0.05"
bg_bright   = "0.020 + clamp(treb * 0.0233, 0, 0.014)"
bg_vignette = "0.80"
trails = "0.66 + clamp(bass * 0.118, 0, 0.10)"

[smoothing]
rotation   = 0.20
thickness  = { attack = 0.03, release = 0.30 }
brightness = { attack = 0.04, release = 0.26 }
glow       = { attack = 0.05, release = 0.55 }
hue        = 0.40
zoom       = 0.25
bg_bright  = 0.40
trails     = 0.50
"#,
    )
    .expect("the pre-repair ridge parses")
}

/// **The non-vacuity check for ADR-0067, and the point of Plan 0058 Phase 1.**
///
/// A gate that cannot fail the defect that motivated it has not been built, so
/// this asserts both halves of the claim on one fixture:
///
/// 1. Under the **old** measurement — the shipped composite, background sampled
///    from pixel (0, 0) — the pre-repair ridge clears the coverage floor and
///    spreads across every quadrant. That is not a re-enactment for colour; it
///    is what let the defect ship, and without it "the new gate fails this"
///    proves nothing about the old one.
/// 2. Under the **new** measurement — backdrop suppressed, compared against
///    black — the same preset scores essentially nothing and fails its floor.
///
/// The gap between the two numbers *is* the vignette. Both captures use the same
/// stimulus, size and frame count, so nothing but the backdrop differs.
#[test]
fn the_pre_repair_ridge_passed_the_old_gate_and_fails_this_one() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = loud();
    let floor = coverage_floor(SystemKind::Spectrum);
    let name = "Spectrum Ridge (pre-repair)";

    // The historical sampler, reproduced here rather than kept in the file: the
    // frame's own top-left pixel as the background reference.
    fn corner(img: &lmv_core::render::CaptureImage) -> [u8; 4] {
        [
            img.rgba.first().copied().unwrap_or(0),
            img.rgba.get(1).copied().unwrap_or(0),
            img.rgba.get(2).copied().unwrap_or(0),
            img.rgba.get(3).copied().unwrap_or(255),
        ]
    }

    // (1) The old gate, backdrop and all.
    renderer.set_presets(vec![pre_repair_spectrum_ridge()]);
    let shipped = renderer
        .capture_preset(name, &frame, FRAMES)
        .expect("capture the pre-repair ridge with its backdrop");
    let bg = corner(&shipped);
    let old_cov = coverage(&shipped, bg, EPS);
    let old_spread = quadrant_spread(&shipped, bg, EPS);
    println!(
        "[pre-repair ridge] old gate: bg={bg:?} coverage={old_cov:.4} (floor {floor:.2}) \
         quadrants={old_spread}"
    );
    assert!(
        old_cov >= floor,
        "the pre-repair ridge must PASS the old corner-sampled gate, or this test proves \
         nothing about why the defect shipped: coverage {old_cov:.4} < {floor:.2}"
    );
    assert!(
        old_spread >= MIN_QUADRANTS,
        "the pre-repair ridge must pass the old spread floor too: {old_spread} quadrant(s)"
    );

    // (2) The new gate: same preset, backdrop suppressed, measured against black.
    renderer.set_presets(vec![without_backdrop(pre_repair_spectrum_ridge())]);
    let scene = renderer
        .capture_preset(name, &frame, FRAMES)
        .expect("capture the pre-repair ridge without its backdrop");
    let cov = coverage(&scene, BLACK, EPS);
    let spread = quadrant_spread(&scene, BLACK, EPS);
    println!(
        "[pre-repair ridge] new gate: coverage={cov:.4} (floor {floor:.2}) quadrants={spread}"
    );
    assert!(
        cov < floor,
        "a contour drawn 3.3 world units off a frame of half-height 1.0 must FAIL the \
         coverage floor once the vignette stops counting as a figure: coverage {cov:.4} \
         >= {floor:.2}"
    );
    assert!(
        old_cov > cov * 10.0,
        "the old gate's score must be dominated by the backdrop, not by the scene: \
         old {old_cov:.4} vs new {cov:.4}"
    );
}
