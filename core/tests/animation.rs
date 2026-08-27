//! Animation liveness (Plan 0013 Phase 3, HARD). A scene must change over time
//! independent of audio: hold the audio frame constant and compare frame N with
//! frame N+k. A frozen scene (e.g. `time` unbound, or a stuck clock) reads as
//! near-zero and fails. Silent audio is used deliberately — the motion under
//! test is the shared scene clock, not an audio edge.
//!
//! **Plan 0077 Phase 1 / ADR-0091: motion is scored against the figure's own
//! footprint, not the whole frame.** The original statistic was
//! `metrics::frame_diff`, a mean over every pixel, which dilutes a sparse
//! figure's motion into the empty frame around it — it scores *occupancy*, and
//! Plan 0067 Phase 1d measured occupancy to be scale-invariant (the ladder at
//! the bottom of this file), so no render size fixes it. The gate now uses
//! `metrics::footprint_diff`: the same mean, taken over the union of lit
//! pixels in the two frames only. The capture roster has its `bg_*` bindings
//! stripped ([`without_backdrop`]) for the same reason sanity.rs strips them
//! (ADR-0067): a shipped backdrop's dim gradient sits above the lit threshold
//! in sRGB, and measured *with* backdrops the sparse probe's footprint read as
//! 65 % of the frame — the dilution this change exists to remove, back again
//! by another door.
//!
//! Software adapter so it holds on any CI GPU.
//!
//! The resolution ladder at the bottom of this file (Plan 0067 Phase 1d) is a
//! **measurement, not a gate** — it is `#[ignore]`d, records why [`SIZE`] did
//! not move, and stays pinned to the whole-frame statistic it measured.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::{Preset, SystemKind, default_presets};
use lmv_core::render::{
    HeadlessOptions, RenderError, Renderer,
    metrics::{footprint_diff, frame_diff},
};

/// Capture size for the gate.
///
/// **Measured against the two designs it penalizes at Plan 0067 Phase 1d and
/// deliberately left alone** — see [`the_resolution_ladder_against_the_two_designs_it_penalizes`]
/// for the ladder and the numbers.
const SIZE: u32 = 96;
/// The earlier and later capture points (frames). The ~0.4 s gap at the fixed
/// 60 fps `SCENE_DT` is ample for any live scene to move visibly.
const FRAME_A: u32 = 24;
const FRAME_B: u32 = 48;
/// Minimum motion between the two frames, in `metrics::footprint_diff`'s
/// mean-abs-over-the-lit-footprint units (0..1). Catches a *frozen* scene, not
/// a subtly-animated one.
///
/// **Re-derived against the footprint statistic at Plan 0077 Phase 1
/// (ADR-0091), from the same measurement the sweep above prints on every run.**
/// The pre-0077 whole-frame floor was also 0.01; the number surviving is a
/// coincidence of the derivation, not the old constant kept. The derivation,
/// per ADR-0071:
///
/// - The shipped library's minimum under the new statistic is **0.0205**
///   (`Banded Mandala`), measured 2026-08-12, DX12 software adapter, backdrop
///   suppressed. The floor sits at **half the shipped minimum** — the sanity
///   suite's per-system-floor convention, applied to this gate's one floor.
///   Slack 2.05x; the sweep prints the distribution, so a new library minimum
///   is re-derived from the printed numbers, not nudged until green.
/// - The noise ceiling sits **below** it: with the mask floored at
///   [`MIN_FOOTPRINT_FRAC`], the worst-case stray event ADR-0091 names (one
///   pixel swinging full-scale on all three channels in an otherwise empty
///   frame) reads `1/139 = 0.0072` — under this floor with 1.4x margin, so a
///   flicker cannot clear a gate that real content clears by 2x.
/// - The non-vacuity pair brackets it: the rejected fifth-density Squall draft
///   reads **0.1049** (10.5x the floor, against 0.0057 under the whole-frame
///   statistic that priced it out), and the static control reads **0.0000** —
///   a zero numerator, which no normalization can rescue (ADR-0091's safety
///   argument). The pair is pinned as a standing test below.
const ANIM_FLOOR: f32 = 0.01;
/// A pixel counts as lit (in the footprint) if any RGB channel differs from
/// [`BLACK`] by more than this — the sanity suite's convention, same value,
/// same reason: shrugs off near-black dithering.
const EPS: u8 = 10;
/// What the footprint is measured against. Not a sampled pixel: the backdrop is
/// suppressed for this capture ([`without_backdrop`]), so every lit pixel is
/// light the **scene** put there.
///
/// **Audited at Plan 0116 Phase 3 and deliberately left constant.** `sanity.rs`
/// now derives its reference per capture (ADR-0126) because a scene that paints
/// its own ground reads `coverage` exactly `1.0` against black. This measure
/// has no single frame to derive from — `footprint_diff` masks over the
/// **union** of two captures, so a per-frame ground would give the mask two
/// references and make the union ill-defined. It is also a *difference*: a
/// scene whose ground is a constant off-white contributes nothing to the
/// numerator whatever the reference calls it, which is the property
/// [`ANIM_FLOOR`] is derived against.
const BLACK: [u8; 4] = [0, 0, 0, 255];
/// Lower bound on `footprint_diff`'s denominator, as a fraction of the frame —
/// the guard ADR-0091 requires against a tiny mask amplifying noise. `0.015` of
/// a 96x96 frame floors the mask at **139 pixels**, which caps the one-pixel
/// full-swing flicker at `0.0072`, under [`ANIM_FLOOR`] (the derivation there).
/// The cost side: a figure lighting less than 1.5 % of the frame has its score
/// scaled down by `mask/139`. The sparsest *silent* footprint in the shipped
/// library is ~7 % of the frame (`Halo` — read any preset's mask fraction off
/// the sweep as `whole-frame / footprint`), 4.5x this bound, so no shipped
/// content is diluted. A future look that sits near this bound is being told
/// it is near-invisible at 96x96 — surface that at the coverage gate rather
/// than tuning this epsilon to pass (ADR-0091's stated risk).
const MIN_FOOTPRINT_FRAC: f32 = 0.015;

/// The prefix every background-stage parameter carries (see sanity.rs, which
/// established the convention and the guard below).
const BG_PREFIX: &str = "bg_";

/// Strip the backdrop bindings so the footprint is the **scene's** lit pixels
/// (ADR-0067's lesson, re-learned here by measurement: a shipped `bg_bright` of
/// 0.016 stores as ~34/255 in sRGB — over [`EPS`] — so with the backdrop on,
/// the "footprint" of a sparse emitter read as 65 % of the frame and the
/// statistic went straight back to diluting by emptiness). The background stage
/// defaults to a black clear, so this is removing bindings, not adding a path.
///
/// One semantic change rides along, deliberately: backdrop drift (a slow
/// `bg_hue` sweep) no longer counts as animation. The gate's question is
/// whether the *scene* moves; a picture whose only life is its vignette
/// breathing is exactly what it should convict.
fn without_backdrop(mut preset: Preset) -> Preset {
    preset.params.retain(|b| !b.name.starts_with(BG_PREFIX));
    preset
}

/// The shipped library, backdrops suppressed, plus `(name, system)` per preset.
/// Panics if the transform matched nothing — the same guard-on-the-guard as
/// sanity.rs: a rename off `bg_` would silently put the backdrop back into the
/// footprint.
fn animation_roster() -> (Vec<Preset>, Vec<(String, SystemKind)>) {
    let mut stripped = 0usize;
    let presets: Vec<Preset> = default_presets()
        .into_iter()
        .map(|p| {
            let before = p.params.len();
            let p = without_backdrop(p);
            stripped += before - p.params.len();
            p
        })
        .collect();
    assert!(
        stripped > 0,
        "no `{BG_PREFIX}*` binding was found in any of the {} shipped presets — the \
         backdrop suppression the footprint statistic rests on has become a no-op",
        presets.len()
    );
    let meta = presets.iter().map(|p| (p.name.clone(), p.system)).collect();
    (presets, meta)
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
        SystemKind::Emitter => "emitter",
        SystemKind::ShapeField => "shape_field",
        SystemKind::WarpMesh => "warp_mesh",
        SystemKind::ShapeCollage => "shape_collage",
    }
}

/// Build a headless `Renderer` at `size`, or `None` (a logged skip) when the
/// runner exposes no GPU adapter — macOS has no software Metal fallback
/// (ADR-0016). Any other build error still panics loudly.
fn headless_at(size: u32) -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: size,
        height: size,
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

/// Both statistics over one pair of captures at silent audio, as
/// `(footprint, whole_frame)`: the gate's footprint difference (ADR-0091) and
/// the pre-0077 whole-frame mean the ladder below stays pinned to.
fn motions(renderer: &mut Renderer, name: &str) -> (f32, f32) {
    let audio = AnalysisFrame::default();
    let early = renderer
        .capture_preset(name, &audio, FRAME_A)
        .expect("capture early frame");
    let late = renderer
        .capture_preset(name, &audio, FRAME_B)
        .expect("capture late frame");
    (
        footprint_diff(&early, &late, BLACK, EPS, MIN_FOOTPRINT_FRAC),
        frame_diff(&early, &late),
    )
}

#[test]
fn every_preset_animates_over_time() {
    let Some(mut renderer) = headless_at(SIZE) else {
        return;
    };

    let (presets, meta) = animation_roster();
    renderer.set_presets(presets);

    let mut failures = Vec::new();
    for (name, system) in meta {
        let (motion, whole) = motions(&mut renderer, &name);
        println!(
            "[{}] {name:<12} frame {FRAME_A} vs {FRAME_B}: footprint {motion:.4} (whole-frame {whole:.4})",
            system_name(system),
        );
        if motion < ANIM_FLOOR {
            failures.push(format!("{name} (motion {motion:.4})"));
        }
    }

    assert!(
        failures.is_empty(),
        "these presets do not animate above {ANIM_FLOOR}: {failures:#?}"
    );
}

/// **The two pinned non-vacuity probes of ADR-0091, as a standing gate** (Plan
/// 0077 Phase 1): the rejected fifth-density Squall draft — the sparse-but-moving
/// design the whole-frame statistic diluted to 0.0051 — must **pass** the
/// footprint statistic, and Phase 1d's static control must keep **failing** it.
/// Together they pin the change to exactly the class ADR-0091 claims it moves:
/// if the draft ever fails, the statistic has regressed to scoring occupancy; if
/// the control ever passes, the floor has stopped gating anything.
///
/// Runs at [`SIZE`] only — the ladder below already measured that resolution
/// does not move either statistic.
#[test]
fn the_footprint_statistic_separates_the_rejected_draft_from_the_static_control() {
    let Some(mut renderer) = headless_at(SIZE) else {
        return;
    };
    let probes = probes();
    let presets: Vec<Preset> = probes
        .iter()
        .map(|(label, _, src)| {
            let p = Preset::from_toml_str(src)
                .unwrap_or_else(|e| panic!("probe `{label}` parses: {e}"));
            without_backdrop(p)
        })
        .collect();
    let names: Vec<String> = presets.iter().map(|p| p.name.clone()).collect();
    renderer.set_presets(presets);

    let mut sparse = None;
    let mut frozen = None;
    for ((label, note, _), name) in probes.iter().zip(names.iter()) {
        let (footprint, whole) = motions(&mut renderer, name);
        println!("{label:<20} footprint {footprint:.4} (whole-frame {whole:.4})   {note}");
        match *label {
            "squall_sparse" => sparse = Some(footprint),
            "star_frozen" => frozen = Some(footprint),
            _ => {}
        }
    }
    let (Some(sparse), Some(frozen)) = (sparse, frozen) else {
        panic!("the probe roster no longer carries the two pinned probes");
    };
    assert!(
        sparse >= ANIM_FLOOR,
        "THE REJECTED DRAFT fails the footprint statistic ({sparse:.4} < {ANIM_FLOOR}) — \
         it has regressed to scoring occupancy (ADR-0091)"
    );
    assert!(
        frozen < ANIM_FLOOR,
        "THE STATIC CONTROL passes the footprint statistic ({frozen:.4} >= {ANIM_FLOOR}) — \
         the floor no longer gates anything"
    );
}

// ---------------------------------------------------------------------------
// The resolution ladder (Plan 0067 Phase 1d) — a measurement, not a gate
// ---------------------------------------------------------------------------

/// Rungs of the ladder. 384 is 16x the pixels of 96, which is why this is not
/// something the shipped-set sweep above can afford to run at.
const LADDER: [u32; 3] = [96, 192, 384];

/// **`Squall` exactly as it shipped**, frozen here when Plan 0075's cohort four
/// retired the file (recover the commented original with
/// `git log --diff-filter=D -- presets/emitter_squall.toml`) — the treatment
/// `ROSETTE_SRC` below prescribed for it.
const SQUALL_SRC: &str = r##"
system = "emitter"
name = "Squall"

[palette]
stops = [
  { at = 0.00, color = "#050a18" },
  { at = 0.28, color = "#1d3f7a" },
  { at = 0.58, color = "#4f8fd6" },
  { at = 0.84, color = "#a9d4f5" },
  { at = 1.00, color = "#f2f8ff" },
]

[params]
spawn_rate = "470 + clamp(bass * 329, 0, 280) + clamp(onset * 424, 0, 254)"
launch_speed = "5.60 + clamp(mid * 0.494, 0, 0.42)"
gravity      = "6.4"
launch_angle = "0.92 + sin(time * 0.061) * 0.13"
spread          = "0.30 + clamp(onset * 0.20, 0, 0.12)"
size_spread     = "0.70"
lifetime_spread = "0.55"
lifetime = "1.7"
size     = "2.05 + clamp(treb * 0.633, 0, 0.38)"
spin    = "0.85 + clamp(mid * 0.741, 0, 0.63)"
twinkle = "0.52 + clamp(treb * 0.467, 0, 0.28)"
brightness = "1.12 + clamp(bass * 0.400, 0, 0.34)"
hue        = "mod(0.60 + time * 0.009, 1)"
hue_spread = "0.38 + clamp(treb * 0.300, 0, 0.18)"
hue_center = "0.58"
saturation = "0.98"
zoom  = "1.00"
pan_x = "sin(time * 0.031) * 0.05"
trails      = "0.44 + clamp(mid * 0.188, 0, 0.16)"
bg_hue      = "0.30 + sin(time * 0.014) * 0.03"
bg_bright   = "0.016 + clamp(treb * 0.0217, 0, 0.013)"
bg_vignette = "0.68"

[smoothing]
spawn_rate   = { attack = 0.05, release = 0.58 }
launch_speed = { attack = 0.10, release = 0.40 }
launch_angle = 0.85
brightness   = { attack = 0.06, release = 0.32 }
size         = { attack = 0.07, release = 0.38 }
hue_spread   = 0.9
trails       = 0.50
bg_bright    = 0.30
"##;

/// **`Star Rosette` exactly as it shipped**, frozen here when Plan 0075's
/// cohort one retired the file (recover the commented original with
/// `git log --diff-filter=D -- presets/star_rosette.toml`). The ladder is a
/// *measurement* whose rungs must stay comparable across runs, so its probe
/// subject is frozen content, not the live library — the same reasoning that
/// froze the sanity suite's mandalas and ADR-0023's golden fixtures.
/// `SQUALL_SRC` above took this same treatment at cohort four.
const ROSETTE_SRC: &str = r#"
system = "star_pattern"
name = "Star Rosette"
[generator]
tiling            = "12"
contact_angle_deg = 20
[params]
variant = "2 * abs(mod(0.5 + time * 0.021 + clamp(bass * 0.41, 0, 0.35), 2) - 1)"
rotation = "0.80 * time + sin(time * 0.031) * 0.26"
draw_progress = "clamp(0.52 + sin(time * 0.20) * 0.42 + bar * 0.20 + clamp(onset * 0.333, 0, 0.20), 0, 1)"
thickness  = "1.70 + clamp(bass * 3.06, 0, 2.6) + beat * 0.40"
brightness = "0.66 + clamp(mid * 0.53, 0, 0.45)"
scale      = "0.58 + sin(time * 0.14) * 0.13 + bar * 0.05 + clamp(bass * 0.47, 0, 0.40)"
hue        = "0.50 + time * 0.023 + clamp(treb * 0.75, 0, 0.45)"
mirror_order   = "select(mid > 0.58, 7, 5)"
mirror_reflect = "0"
zoom        = "1.06 + bar * 0.06 + clamp(bass * 0.176, 0, 0.15)"
pan_x       = "sin(time * 0.0170) * 0.05"
bg_hue      = "0.10 + sin(time * 0.0091) * 0.06"
bg_bright   = "0.018 + clamp(mid * 0.0153, 0, 0.013)"
bg_vignette = "0.82"
trails = "0.34 + clamp(mid * 0.082, 0, 0.07)"
[smoothing]
mirror_order  = 2.5
variant       = 1.5
rotation      = 0.18
thickness     = { attack = 0.02, release = 0.34 }
scale         = 0.55
hue           = 0.42
zoom          = 0.18
bg_bright     = 0.40
trails       = 0.55
"#;

/// A 12-fold Hankin rosette whose **only** motion is rotation — design-backlog
/// 0009's symmetric case, isolated. `star_rosette` as shipped works around the
/// problem by sweeping `draw_progress` radially and breathing `scale`; this
/// probe removes the workaround so what is measured is the rotation alone.
const STAR_SPIN: &str = r#"
system = "star_pattern"
name   = "probe_star_spin"

[generator]
tiling            = "12"
contact_angle_deg = 20

[params]
variant       = "1"
rotation      = "0.80 * time"
hue           = "0.5"
draw_progress = "1"
thickness     = "1.7"
scale         = "0.58"
brightness    = "0.9"
"#;

/// The static control: [`STAR_SPIN`] with the rotation taken out, so nothing in
/// it reads `time` or any band. It must keep failing the floor at every rung —
/// a resolution that lets *this* through would have made the gate worthless.
const STAR_FROZEN: &str = r#"
system = "star_pattern"
name   = "probe_star_frozen"

[generator]
tiling            = "12"
contact_angle_deg = 20

[params]
variant       = "1"
rotation      = "0"
hue           = "0.5"
draw_progress = "1"
thickness     = "1.7"
scale         = "0.58"
brightness    = "0.9"
"#;

/// The `key = "<expr>"` line of a preset source, split into (line, expr).
/// Panics when the key is absent, so a rename upstream fails this probe loudly
/// instead of silently measuring the shipped preset twice.
fn param_line<'a>(src: &'a str, key: &str) -> (&'a str, &'a str) {
    let line = src
        .lines()
        .find(|l| {
            l.trim_start()
                .strip_prefix(key)
                .is_some_and(|rest| matches!(rest.trim_start().chars().next(), Some('=')))
        })
        .unwrap_or_else(|| panic!("`{key}` is not a param of this preset source"));
    let expr = line
        .split_once('=')
        .map(|(_, rest)| rest.trim().trim_matches('"'))
        .unwrap_or_else(|| panic!("`{key}` has no value"));
    (line, expr)
}

/// Rewrite one `key = "..."` line's value, keeping the rest of the file byte for
/// byte. The probes are built out of the shipped sources this way so they cannot
/// drift into being a different preset than the one 0009 is about.
fn with_param(src: &str, key: &str, value: &str) -> String {
    let (line, _) = param_line(src, key);
    src.replace(line, &format!("{key} = \"{value}\""))
}

/// The five probes, as `(label, note, source)`.
fn probes() -> Vec<(&'static str, &'static str, String)> {
    let (_, spawn) = param_line(SQUALL_SRC, "spawn_rate");
    let sparse = with_param(
        &with_param(SQUALL_SRC, "spawn_rate", &format!("({spawn}) * 0.2")),
        "name",
        "probe_squall_sparse",
    );
    let squall = with_param(SQUALL_SRC, "name", "probe_squall_shipped");
    let rosette = with_param(ROSETTE_SRC, "name", "probe_rosette_shipped");
    vec![
        (
            "squall_shipped",
            "the density that clears the floor",
            squall,
        ),
        (
            "squall_sparse",
            "THE REJECTED DRAFT — spawn_rate at a fifth",
            sparse,
        ),
        (
            "rosette_shipped",
            "the symmetric design, with its radial workaround",
            rosette,
        ),
        (
            "rosette_spin_only",
            "THE SYMMETRIC CASE — rotation is the only motion",
            STAR_SPIN.to_string(),
        ),
        (
            "star_frozen",
            "STATIC CONTROL — must fail at every rung",
            STAR_FROZEN.to_string(),
        ),
    ]
}

/// **The Plan 0067 Phase 1d measurement, and its answer: resolution does not
/// separate a sparse-but-moving frame from a static one.** `#[ignore]`d because
/// it is an instrument rather than a gate — the 384 rung alone is 16x the pixels
/// of the sweep above. Run it with:
///
/// ```bash
/// cargo nextest run -p lmv-core --test animation --run-ignored all --no-capture
/// ```
///
/// Measured 2026-08-09, DX12 software adapter (`frame_diff`, frames 24 vs 48,
/// silent audio; `ANIM_FLOOR` is 0.01). Whole run: **7.6 s**.
///
/// ```text
/// probe                    96      192      384   change over 16x the pixels
/// squall_shipped       0.0187   0.0187   0.0188   +0.0001
/// squall_sparse        0.0051   0.0051   0.0049   -0.0002   THE REJECTED DRAFT
/// rosette_shipped      0.0218   0.0212   0.0208   -0.0010
/// rosette_spin_only    0.0103   0.0100   0.0103    0.0000   THE SYMMETRIC CASE
/// star_frozen          0.0000   0.0000   0.0000    0.0000   STATIC CONTROL
/// ```
///
/// **Every row is flat.** Across a 16x change in pixel count no probe moves by
/// more than 0.001, and none crosses the floor. The two rows the phase exists
/// for — the rejected sparse draft at 0.005 and the isolated symmetric case at
/// 0.010 — sit exactly where they sat at 96. Resolution is not the lever.
///
/// **Half of that was settled before a pixel was rendered.** A figure invariant
/// under rotation by `2*pi/k` renders an identical image under that rotation, so
/// its whole-frame difference is zero at *every* resolution and no ladder could
/// rescue it. `rosette_spin_only` is the empirical half: a 12-fold rosette turns
/// ~18 degrees between the two capture points, which is not a symmetry of the
/// figure, so it does not read zero — it reads 0.0103, clearing the floor by
/// 3 % where the shipped `star_rosette` clears it by 118 %. The penalty
/// design-backlog 0009 describes is real and it is roughly a halving; what it is
/// not is a sampling artifact.
///
/// **The sparse case was the one plausibly resolution-bound, and it is not.**
/// The hypothesis was that a mark smaller than a pixel at 96x96 is lost rather
/// than area-averaged, so rendering larger would recover its motion. Squall's
/// rejected draft reads 0.0051 at 96 and 0.0049 at 384 — very slightly *worse*,
/// which is the mechanism showing through: `frame_diff` is a **mean over the
/// frame**, so the marks and the dark they are averaged against grow together.
/// Both of 0009's designs fail for the same reason and it is not resolution — a
/// mean-over-the-frame statistic scores **occupancy**, and occupancy is
/// scale-invariant.
///
/// **So neither [`SIZE`] nor [`ANIM_FLOOR`] moves, and the CI cost of that choice
/// is zero.** 192 would have cost the whole-library sweep 4x the pixels and 384
/// 16x, for a reading that changes in the third decimal place.
///
/// **One thing the table shows that is deliberately not acted on.** The sparse
/// draft (0.0051) and the static control (0.0000) *are* separated, so a floor
/// near 0.004 would pass the draft and still convict the control. That is not a
/// finding this phase may spend: the control here is a *perfectly* frozen figure
/// and a real near-frozen preset lands somewhere between the two, so the gap is
/// an upper bound on the headroom rather than the headroom. 0009 says the same
/// thing in its own words — "a floor that a genuinely static preset can clear is
/// worth nothing" — and lowering it on one synthetic control would be exactly
/// the blind give-away it warns against.
///
/// Per the phase, this is a **successful negative result**: it forecloses the
/// cheap fix and leaves standing 0009's real successor — a coverage-aware
/// statistic that normalizes motion by the **lit area** rather than by the frame.
/// That was explicitly out of this plan's scope; this table is the evidence that
/// it is now the only remaining option.
///
/// **That successor landed at Plan 0077 Phase 1 (ADR-0091)** — the gate above
/// now scores `metrics::footprint_diff`. This ladder deliberately keeps
/// measuring the whole-frame statistic it is pinned to: its table is the
/// recorded evidence for why the statistic had to change, and re-pointing the
/// instrument at the new statistic would erase the comparison the numbers
/// exist to hold. Under the footprint statistic the rejected draft reads
/// 0.1049 against the shipped 0.1093 — the density difference cancels, which
/// is the whole claim — and the static control keeps its zero numerator.
///
/// **Being `#[ignore]`d has a cost worth naming:** the two probes rebuilt from
/// shipped sources go through [`param_line`], which panics if `spawn_rate` or
/// `name` is renamed — and a panic in an ignored test is silent. If this is run
/// after a preset edit and it panics, the probe needs updating, not the preset.
#[test]
#[ignore = "measurement, not a gate: the 384 rung is 16x the pixels of the sweep"]
fn the_resolution_ladder_against_the_two_designs_it_penalizes() {
    let probes = probes();
    let mut rows: Vec<(&str, &str, Vec<f32>)> = probes
        .iter()
        .map(|(label, note, _)| (*label, *note, Vec::new()))
        .collect();

    for size in LADDER {
        let Some(mut renderer) = headless_at(size) else {
            return;
        };
        let presets: Vec<Preset> = probes
            .iter()
            .map(|(label, _, src)| {
                Preset::from_toml_str(src).unwrap_or_else(|e| panic!("probe `{label}` parses: {e}"))
            })
            .collect();
        let names: Vec<String> = presets.iter().map(|p| p.name.clone()).collect();
        renderer.set_presets(presets);
        for (row, name) in rows.iter_mut().zip(names.iter()) {
            row.2.push(motions(&mut renderer, name).1);
        }
    }

    print!("\n{:<20}", "probe");
    for size in LADDER {
        print!("{size:>9}");
    }
    println!("   note");
    for (label, note, values) in &rows {
        print!("{label:<20}");
        for v in values {
            print!("{v:>9.4}");
        }
        println!("   {note}");
    }
    println!("\n(ANIM_FLOOR = {ANIM_FLOOR})");

    // The one thing this measurement must not be allowed to be quietly wrong
    // about: if the static control ever animates, every row above is noise.
    let frozen = rows
        .iter()
        .find(|(label, _, _)| *label == "star_frozen")
        .map(|(_, _, v)| v.clone())
        .unwrap_or_default();
    assert!(
        frozen.iter().all(|&v| v < ANIM_FLOOR),
        "the static control animates at some rung — the ladder is measuring \
         something other than motion: {frozen:?}"
    );
}
