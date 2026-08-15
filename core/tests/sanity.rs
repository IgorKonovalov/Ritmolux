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
//! Coverage floors stay per-system, because the systems still differ by an order
//! of magnitude in how much they paint — `fragment_field` fills the frame while
//! `spectrum` draws a contour — so a single broad floor would be either
//! tautological for one or impossible for the other. **The floors themselves were
//! all re-derived in Plan 0058 Phase 2**, since every one of them had been
//! measured through a backdrop. They are set at half each system's lowest shipped
//! preset and [`MAX_FLOOR_SLACK`] keeps them there; the old note that the `swarm`
//! is "sparse points" was measurement folklore, and it measures `0.84`.
//!
//! **Plan 0058 Phase 3 adds a second excitation.** Every question above is asked
//! of one fully-driven frame, which cannot see a figure that is fine at rehearsal
//! level and gone at the top of its range.
//! [`a_louder_frame_is_reported_against_a_quieter_one`] captures the library at
//! [`MODERATE`] as well as [`LOUD`] and reports the ratio between them. It is a
//! **report, not a gate** — the measurement in its doc comment is the argument,
//! and the short version is that no threshold on that axis convicts any of the
//! three known defective configurations while the nearest content to one is the
//! attractor family's deliberate idiom. The second capture does buy one gate:
//! [`MODERATE_MIN_COVERAGE`], that a preset is a picture at a realistic level.
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
        metrics::{
            RADIAL_SHELLS, TONE_BANDS, coverage, quadrant_spread, radial_shell_occupancy,
            tonal_flatness,
        },
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
/// **Re-measured under ADR-0067** (backdrop suppressed, compared against
/// [`BLACK`]), because removing the backdrop changes which pixels are counted at
/// all — and it changed this statistic more than it changed any other:
///
/// ```text
///          this measurement        previously (corner-sampled)
/// 0.8839   Rose Web                0.8655  Spectrum Ridge
/// 0.7211   Rose Trails             0.8300  Rose Trails
/// 0.6755   Ink on Paper            0.7645  Rose Web
/// 0.4249   Cathedral               0.6588  Coral
/// 0.4107   Aurora                  0.6438  De Jong
/// 0.3604   Supernova               0.4923  Coral Head
/// 0.3147   Leviathan               0.4518  Coral Bloom
/// 0.3014   Rose Overflow           0.4453  Leviathan
/// ```
///
/// **The value did not move and the two numbers behind it did.** Read the
/// columns against each other rather than down:
///
/// - `Spectrum Ridge` fell `0.8655` → **`0.1916`**, off the table entirely. It
///   was never a flat *preset*; it was a lit `bg_vignette` measured as if it were
///   one, which is [`KNOWN_FLAT`]'s note and the whole of ADR-0067's case. `Coral`
///   (`0.6588` → `0.2681`) and `De Jong` (`0.6438` → `0.2606`) were the same
///   error at lower amplitude.
/// - `Rose Web` went the other way, `0.7645` → `0.8839`, and now tops the
///   distribution. Nothing about the preset changed. The vignette had been
///   contributing a broad spread of mid-tones that diluted the share sitting in
///   any one band; with it gone, what is left is the figure, and a web of
///   near-equal-brightness strokes genuinely has very little tonal structure.
///   The number is worse because it is now honest.
///
/// **So the plan's question — does re-measuring widen the `0.035` margin — is
/// answered no, and it is a finding rather than a reason to move the constant.**
/// The margin above the library is now **`0.0161`** (`0.90` over `Rose Web`'s
/// `0.8839`), narrower than before. Below, the deliberately flattened fixture
/// reads `0.9815`, so `0.90` still separates the library from the fixture, but it
/// sits `0.0161` above one and `0.0915` below the other — not a midpoint.
/// `0.90` is left where it is because the margin narrowed for a *real* reason:
/// the top of the distribution is now an actual figure with an actual tonal
/// problem, and a preset drifting over the ceiling is a preset to route, not a
/// constant to nudge.
///
/// The top three are structural rather than accidental — a trails-heavy line
/// look is mostly faint tail at one level, a web is mostly stroke, and
/// `Ink on Paper` is a deliberate two-tone ink remap. Under [`loud`] every band
/// is driven to `1.0` at once, which is the worst possible stimulus for exactly
/// those shapes.
///
/// So do not read a pass here as headroom. A measured constant with a shelf
/// life: re-measure when the library changes materially, and if the top of the
/// distribution keeps climbing, the thing to question is whether an all-bands-up
/// stimulus can fairly judge a figure made of near-equal strokes — not whether to
/// nudge `0.90`.
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
///
/// Plan 0058 settled that reading with a number: under the ADR-0067 measurement
/// the repaired `Spectrum Ridge` reads **`0.1916`**, not `0.8655`. Almost all of
/// what this list was once tracking was the backdrop. The list stays **empty** —
/// Phase 1's change put no preset back over the ceiling, and if one ever goes
/// over, that is a defect to route, not an entry to re-add.
const KNOWN_FLAT: &[&str] = &[];

/// The fewest of [`RADIAL_SHELLS`] concentric annuli a preset under its
/// system's coverage floor must occupy to pass anyway — the **structural
/// rescue** (Plan 0075 Phase 1, design-backlog 0072).
///
/// # Why a structural measure and not a per-family floor
///
/// Backlog 0072 proved the coverage statistic cannot see a dense thin-stroke
/// figure at this test's 96×96: the bare rosette and a 46×-denser four-ring
/// mandala score **identically** (0.403 in a controlled A/B), 54 % more
/// geometry moves the number 2.6 %, and the only lever that clears the floor is
/// inflating `glow`/`trails` — the washed-out look the user rejected in the
/// running app. **The gate was selecting for the defect.** The entry named two
/// candidate mechanisms, and this file takes the structural one, for two
/// reasons recorded here rather than re-derived:
///
/// 1. **A per-family thin-stroke floor is still the halo-meter, recalibrated.**
///    Lowering `star_pattern`'s floor to what honest thin strokes render
///    (`0.2442 / 2 = 0.12`) keeps measuring the halo — it just demands less of
///    it — so the next *denser* mandala meets the same wall at a new number.
///    It also cannot coexist with [`MAX_FLOOR_SLACK`]: the shipped family
///    minimum is `0.6908` (Star Lantern), and a defect-shaped `0.12` floor sits
///    `5.75×` under it, which [`report_coverage_distribution`] rightly fails.
///    The floor machinery is calibrated to *shipped* content; the defect lives
///    in content that could not ship because of it.
/// 2. **Shell occupancy sees the figure, and cannot be bought with glow.** The
///    Plan 0065 lane's prototype separated the pair the coverage statistic
///    could not: 9 of 10 radial shells occupied against 1. Inflating the halo
///    around a stroke does not move which shells the stroke lives in, so the
///    rescue removes the incentive the old floor created instead of repricing
///    it.
///
/// # How it applies
///
/// A preset under its coverage floor is not failed if it occupies at least this
/// many shells. Everything else stands unchanged: the quadrant check still
/// fails a dot, [`MAX_TONAL_FLATNESS`] still fails a blot, the floors and
/// [`MAX_FLOOR_SLACK`] still run against shipped content (which all clears them
/// — no shipped preset needs the rescue today), and a scene that renders
/// **nothing** occupies zero shells and still fails, which is the one job the
/// old floor demonstrably did
/// ([`the_pre_repair_ridge_passed_the_old_gate_and_fails_this_one`] asserts it
/// on the frozen defect).
///
/// # Derivation
///
/// Half the sparsest legitimate content, the same ceremony every coverage floor
/// in this file follows (ADR-0071: a constant states its derivation next to
/// itself). The honest mandala tunings — the three retired presets at
/// `glow = 1.0` with no `trails`, frozen from `654304a^` below — measure
/// **10 / 10 / 9** of 10 shells
/// ([`the_honest_mandala_tunings_pass_the_structural_measure`] prints them on
/// every run), so the bar is `9 / 2 = 4` (integer floor, the conservative
/// side), against defect fixtures that measure exactly `0`. A figure occupying
/// four annuli is structurally *present* — what it may still be is washed out,
/// mis-toned or dot-like, and those remain the other checks' verdicts.
///
/// This is a rescue bar, **not** a general structural floor over the library:
/// shipped presets that clear their coverage floors legitimately sit under it
/// (`Spectrum Ridge` occupies 3 shells, `Rose Trails` and `Spectrum Corona`
/// 5 — the main gate prints every preset's count). A sparse readout that
/// passes on coverage never consults this number.
const MIN_STRUCTURAL_SHELLS: usize = 4;

/// The most the lowest-scoring preset in a system may sit above that system's
/// floor before the floor has stopped doing anything (Plan 0058 Phase 2).
///
/// The old floors were `0.01` for six of the eight systems against a library
/// whose sparsest member now measures `0.1189` — a factor of **11.9**, and on
/// the two systems above it a factor of 24 to 84. Nothing could fail them except
/// a literally black frame, which is why they survived a preset drawn entirely
/// off-frame. A floor is only a floor if the content is somewhere near it.
///
/// Enforced by [`report_coverage_distribution`], which is the mechanism that
/// gives this paragraph a shelf life instead of a good intention. It fires when
/// the *lowest* preset in a system rises well clear of the floor — retuning or
/// retiring the sparsest member of a family — and the fix is to re-measure that
/// floor from the distribution the gate prints, never to leave the slack in.
const MAX_FLOOR_SLACK: f32 = 2.2;

/// Per-system minimum lit fraction, **measured from the shipped library** under
/// the ADR-0067 measurement (backdrop suppressed, compared against [`BLACK`]).
///
/// Each floor is set at half its system's lowest shipped preset, so the gap is a
/// factor of ~2 everywhere and [`MAX_FLOOR_SLACK`] holds it there. The full
/// distribution is printed by `every_preset_draws_a_real_shape` on every run;
/// the lowest member and the resulting factor per system:
///
/// ```text
/// system              floor   lowest preset            factor
/// fragment_field      0.50    0.9926  Kaleido Field      1.99
/// swarm               0.42    0.8407  Storm              2.00
/// parametric_curve    0.33    0.6722  Rose Trails        2.04
/// lsystem             0.50    1.0000  Vellum             2.00
/// star_pattern        0.34    0.6908  Star Lantern       2.03
/// reaction_diffusion  0.09    0.1910  Verdigris          2.12
/// attractor           0.18    0.3442  Leviathan          1.91
/// spectrum            0.28    0.5843  Halo               2.09
/// ```
///
/// **This table is a snapshot and the printed distribution is authoritative.**
/// As of 2026-08-06 three rows have drifted *upward* without any floor needing
/// to move — `swarm`'s minimum is now `0.6208` Starfield (slack `1.48`),
/// `attractor`'s is `0.2356` Lorenz (`1.31`), and `emitter` postdates the table
/// entirely at `0.25` against `0.3086` Squall (`1.23`). Upward drift is the safe
/// direction and [`MAX_FLOOR_SLACK`] is what will eventually call it; the rows
/// are left as written because the prose below narrates the numbers in them.
///
/// **The `star_pattern` floor went to `0.12` on 2026-08-06 and came back to
/// `0.34` the same day, and both moves were correct.** Plan 0065 filled the
/// interior of that scene with three ring-mandala presets measuring `0.2442` /
/// `0.2505` / `0.2544`, which moved the family minimum below the floor, so the
/// floor was re-derived to `0.2442 / 2 = 0.12` exactly as the rule below says to.
/// Then the user rejected all three on sight in the running app and they were
/// **retired**, taking the minimum back to `0.6908` Star Lantern and the floor
/// back to `0.34`.
///
/// **Keep the round trip in mind before treating a re-derivation as permanent.**
/// The reason the presets were cut is not the reason the floor moved: every motif
/// in that scene is a parametric outline *sampled to straight segments*, so the
/// vertices are visible and a circle reads as a polygon — a ceiling on the
/// approach, not a tuning miss (design-backlog 0073). The mandala look now ships
/// as `reaction_gilt`, an analytic iso-contour field folded by `kaleido_order`,
/// where there is no geometry and so no vertex at any resolution.
///
/// **What the episode still proves about this measure stands, and it is filed.**
/// At this test's 96x96 capture a hairline over a 46-fold ornament aliases to
/// almost nothing, so `coverage` on that content measured the halo and the trail
/// rather than the figure — the bare rosette and the 46x-denser mandala scored
/// *identically*, and 54 % more geometry moved the number 2.6 %. See
/// design-backlog 0072, which stays open at medium-high and asks for a structural
/// occupancy measure; nothing about retiring the presets answers it.
///
/// **The attractor floor moved on 2026-08-03 and the mechanism above is why it
/// was noticed rather than missed.** It was `0.12` against `0.2461` De Jong.
/// Plan 0057 Phase 6 re-raised the exposure of Clifford, De Jong and Leviathan,
/// undoing a compensation `00d99d0` had carried for a 3x deposit that ADR-0065
/// removed, and the family's minimum rose to `0.3785`, moving from De Jong to
/// **Leviathan**. That put the slack at `3.15x` and
/// [`report_coverage_distribution`] failed the run with the number, which is
/// exactly the shelf life this constant was given. The floor is re-derived
/// from the printed distribution, not nudged until green. The family read
/// `0.3785 Leviathan`, `0.4746 De Jong`, `0.5381 Lorenz`, `0.7817 Clifford`,
/// `1.0000 Ink on Paper`, `1.0000 Thomas`.
///
/// **Every attractor number above fell on 2026-08-04 (Plan 0059 Phase 1b) and the
/// floor did not have to move.** ADR-0070 stopped the trail sampling its own
/// target mirrored, so each figure is one copy where it used to be `figure ∪
/// mirror(figure)` — strictly less lit area, by 6-13 %. The family now reads
/// `0.3442 Leviathan`, `0.4480 De Jong`, `0.5268 Lorenz`, `0.6831 Clifford`,
/// `1.0000 Ink on Paper`, `1.0000 Thomas`, putting the slack at `1.91x`. It is
/// recorded rather than acted on because the floor is still inside
/// [`MAX_FLOOR_SLACK`], and Plan 0059 Phase 4 re-authors this whole family
/// against the un-doubled figure — re-deriving the constant now would set it from
/// exposure nobody has judged yet.
///
/// Reaction-diffusion is unchanged here **and that is a result, not an
/// omission**: Phase 1b moved its three passes to the same prelude, which
/// reversed the direction a positive `pan_y` scrolls the field and moved `Coral`
/// to `0.1546` (slack `2.21x`, failing this gate by a hair). The phase restored
/// the sign rather than re-measuring the floor, and `Coral` returned to `0.1420`
/// exactly — which is the evidence that the reversal, not the un-mirroring, was
/// what moved it.
///
/// Two of that six deserve a note, because a reader checking the arithmetic
/// will trip on them. `Ink on Paper` and `Thomas` both read exactly `1.0000`,
/// and for `Ink` that is a **measurement artifact rather than a saturated
/// figure**: it sets `ink_amount = 1`, and the ink remap is a terminal engine
/// stage, not a `bg_*` binding, so ADR-0067's backdrop suppression does not
/// reach it - the whole frame is paper-white and every pixel differs from
/// [`BLACK`]. Its tonal flatness (`0.6756`) is the statistic that actually
/// describes it. `Lorenz` was deliberately left un-retuned pending Plan 0057
/// Phase 5, so its `0.5381` is a pre-retune number and will move again.
///
/// **These numbers replace floors that could not be failed.** Under the old
/// corner-sampled measurement the same six sparse systems all read `0.01` and
/// `bg_vignette` cleared that on its own; the pre-repair `spectrum_ridge` scored
/// `0.5421` while drawing nothing at all
/// (`the_pre_repair_ridge_passed_the_old_gate_and_fails_this_one`). Re-deriving
/// them was not optional bookkeeping — a floor derived from inflated numbers is
/// not a floor (ADR-0067).
///
/// **What the factor of 2 costs, stated rather than discovered.** This is
/// deliberately the sensitive end of the range, unlike
/// [`SATURATED_OCCUPANCY`](lmv_core::preset::SATURATED_OCCUPANCY), which took a
/// wide margin because a HARD gate that fires on good content buys exemptions.
/// The difference is what "wrong" looks like on each side: an over-driven clamp
/// is a *number* that stopped moving and a generous threshold still catches it,
/// whereas an off-frame figure is a *picture that is not there*, and the sparsest
/// legitimate content in this library still paints twice the floor. A new preset
/// that fails one of these has drawn less than half of what the thinnest shipped
/// member of its own family draws, which is worth a look even when it turns out
/// to be fine.
///
/// Three families vary internally by 3-5x (`attractor` 0.2461-1.0000, `spectrum`
/// 0.1189-0.5541, `reaction_diffusion` 0.1420-0.4427), so their floors sit over
/// the most movement and are the ones most likely to need a re-measure. The
/// response to a legitimately sparser new preset is to re-derive that system's
/// floor from the printed distribution, and to say in the commit which preset
/// moved the minimum — not to nudge a constant back until the run goes green.
fn coverage_floor(system: SystemKind) -> f32 {
    match system {
        // Full-screen field: every shipped member is above 0.99, so the spread
        // is 0.0074 wide and anything near this floor is a broken field.
        SystemKind::FragmentField => 0.50,
        // A dense point cloud that fills the frame far more than "sparse points"
        // suggested — the old 0.01 was 84x below the thinnest of the three.
        SystemKind::Swarm => 0.42,
        // Line art. The trails-heavy looks score lowest because a faint tail is
        // still lit; Rose Trails at 0.6722 sets this one.
        SystemKind::ParametricCurve => 0.33,
        // Raised from 0.32 on 2026-08-13 when `Wildwood` was retired on sight in
        // the running app: it was the family minimum, and its removal left
        // `Vellum` at 1.0000 as the only shipped member, putting the old floor
        // 3.12x below it — over this file's 2.2x slack. Re-derived at half the
        // new minimum like every floor here. A one-member family means the next
        // lsystem preset will very likely move this number again.
        SystemKind::LSystem => 0.50,
        // Went to 0.12 and back on 2026-08-06 — see the doc comment. Star
        // Lantern's 0.6908 sets it again now that the three ring mandalas are
        // retired.
        SystemKind::StarPattern => 0.34,
        // Reaction-diffusion paints a real pattern across the frame, but the
        // present maps only the sparse V species, so the lit fraction is modest.
        // Raised from 0.07 when cohort three (Plan 0075) retired the corals:
        // the family minimum moved up to Verdigris at 0.1910, and the old
        // floor sat 2.73x below it — over this file's 2.2x slack.
        SystemKind::ReactionDiffusion => 0.09,
        // The attractor cloud is the widest-spread family: Leviathan's sheet at
        // 0.3785 against two members that fill the frame. Raised from 0.12 at
        // Plan 0057 Phase 6 — see the table above for why the minimum moved
        // off De Jong.
        SystemKind::Attractor => 0.18,
        // The sparsest system in the library, and the one this plan exists
        // because of. Spectrum Ridge sets it at 0.1189 — *after* its repair; the
        // version that shipped broken scores 0.0000 here.
        // Raised from 0.06 when cohort four (Plan 0075) retired the three
        // spectrum presets: the family minimum moved up to Halo at 0.5843
        // (its lit violet atmosphere and thick spokes cover what the old
        // thin combs did not), and the old floor sat 9.7x below it.
        SystemKind::Spectrum => 0.28,
        // A shower of small marks over an otherwise empty frame — sparse by
        // idiom, like the spectrum readout and unlike the swarm's dense cloud.
        // Measured from `Sparks`, the family's only shipped member (Plan 0052),
        // and set at half of it like every floor above. When a second emitter
        // preset lands this is the number to re-derive from the distribution
        // this test prints.
        SystemKind::Emitter => 0.25,
        // **Not derived from a distribution, because there is no distribution
        // yet**: Plan 0091 ships the `shape_field` engine and deliberately no
        // preset content (ADR-0081 puts worlds in the author's lane), so this
        // family has zero shipped members and this floor has never gated
        // anything. It is inherited from `FragmentField` on the structural
        // argument alone — both are fullscreen scenes that cover every pixel
        // with `occlude`, so a shape field that is not broken cannot score low.
        // **Re-derive it from this test's printed distribution when the first
        // one ships**, at half the family minimum like every floor above.
        SystemKind::ShapeField => 0.50,
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
        SystemKind::Emitter => "emitter",
        SystemKind::ShapeField => "shape_field",
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

/// A sustained frame with every level driven to `level` and a beat, so any
/// audio-gated brightness reaches its lit state.
///
/// "Every band up" includes the `spectrum` array itself (Plan 0034 Phase 2). A
/// frame with `bass = mid = treb = 1.0` and 64 silent log-bands is not a frame
/// any audio could produce, and under it a spectrum readout would correctly draw
/// almost nothing — the floor would be measuring the fixture, not the scene. No
/// pre-0034 scene reads `spectrum`, so every other preset's capture is
/// unchanged.
///
/// `beat` and `bar` are held **constant** across levels (Plan 0058 Phase 3): the
/// excitation ratio has to vary one thing, and a beat-latched figure that appears
/// only on the beat would otherwise swamp the level's own contribution.
fn excited(level: f32) -> AnalysisFrame {
    AnalysisFrame {
        bass: level,
        mid: level,
        treb: level,
        onset: level,
        beat: true,
        bar: 0.5,
        spectrum: [level; lmv_core::dsp::SPECTRUM_BINS],
        ..Default::default()
    }
}

/// The two excitations Phase 3 compares, and the drive every other test here
/// uses ([`LOUD`], the fully-driven frame this file has always rendered).
const LOUD: f32 = 1.0;
/// A realistic mid-track level rather than a whisper. Low enough that a
/// world-space param driven past the frame at [`LOUD`] is still comfortably
/// inside it here, high enough that every audio-gated brightness is already lit
/// — the comparison must isolate *how far* a figure is driven, not whether it
/// switched on at all.
const MODERATE: f32 = 0.4;

/// A sustained "loud" frame: every band up and a beat.
fn loud() -> AnalysisFrame {
    excited(LOUD)
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
    let mut by_system: Vec<(SystemKind, f32, String)> = Vec::new();
    for (name, system) in &meta {
        let (name, system) = (name.as_str(), *system);
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture preset");
        let cov = coverage(&img, BLACK, EPS);
        let spread = quadrant_spread(&img, BLACK, EPS);
        let flat = tonal_flatness(&img, BLACK, EPS);
        let shells = radial_shell_occupancy(&img, BLACK, EPS);
        let floor = coverage_floor(system);
        println!(
            "[{}] {name:<12} coverage={cov:.4} (floor {floor:.2}) quadrants={spread} \
             flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2}) shells={shells}/{RADIAL_SHELLS}",
            system_name(system),
        );
        let known_flat = KNOWN_FLAT.contains(&name);
        flatness.push((flat, name.to_string(), known_flat));
        by_system.push((system, cov, name.to_string()));
        if cov < floor {
            // The structural rescue (Plan 0075 Phase 1): a dense thin-stroke
            // figure aliases to almost no coverage at this capture size, so a
            // preset under its floor is asked the structural question before
            // being convicted — see MIN_STRUCTURAL_SHELLS for the mechanism
            // and the derivation.
            if shells >= MIN_STRUCTURAL_SHELLS {
                println!(
                    "  {name} is under its coverage floor ({cov:.4} < {floor:.2}) but \
                     structurally present: {shells}/{RADIAL_SHELLS} radial shells occupied \
                     (min {MIN_STRUCTURAL_SHELLS}) — a thin-stroke figure, not a blank frame"
                );
            } else {
                failures.push(format!(
                    "{name} blank: coverage {cov:.4} < {floor:.2} and only \
                     {shells}/{RADIAL_SHELLS} radial shells occupied \
                     (min {MIN_STRUCTURAL_SHELLS})"
                ));
            }
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

    // The two distributions every constant in this file is set from, printed on
    // every run so the next re-measurement does not need a special one — and, for
    // the coverage floors, checked rather than only printed.
    failures.extend(report_coverage_distribution(&by_system));

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

/// Print each system's coverage distribution against its floor, lowest first,
/// with the factor between the floor and that system's lowest preset — and
/// return a failure for every floor that factor has left behind.
///
/// **The floors are only floors while the content is near them** (Plan 0058
/// Phase 2). This is what stops the re-derivation in [`coverage_floor`] from
/// decaying back into the state it replaced. The old `0.01` did not start out
/// useless — it stopped being useful as the library grew denser and nothing was
/// watching, and by the time a preset drew nothing at all the floor was 11.9x
/// below the sparsest thing that could fail it. A comment saying "re-measure when
/// the library changes materially" did not survive that; this does, because it
/// fails the build.
///
/// It cannot fire on a *new sparse* preset — that case fails the coverage floor
/// itself, loudly and by name. It fires only when the sparsest member of a family
/// is retuned upward or retired, which is exactly when a re-measure is owed.
///
/// It runs off the captures the caller already took. A second sweep would be 35
/// more WARP renders to recompute numbers that are already in hand.
fn report_coverage_distribution(by_system: &[(SystemKind, f32, String)]) -> Vec<String> {
    let mut slack = Vec::new();
    println!("coverage by system (floor, then every preset lowest-first):");
    for system in SystemKind::ALL {
        let mut rows: Vec<(f32, &str)> = by_system
            .iter()
            .filter(|(s, ..)| *s == system)
            .map(|(_, cov, name)| (*cov, name.as_str()))
            .collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        let floor = coverage_floor(system);
        let (lowest, lowest_name) = rows[0];
        let factor = lowest / floor;
        println!(
            "  {:<18} floor {floor:.2}  lowest {lowest:.4} ({lowest_name}) — factor \
             {factor:.2} (max {MAX_FLOOR_SLACK:.1})",
            system_name(system),
        );
        let listed: Vec<String> = rows
            .iter()
            .map(|(cov, name)| format!("{cov:.4} {name}"))
            .collect();
        println!("      {}", listed.join("  |  "));

        if factor > MAX_FLOOR_SLACK {
            slack.push(format!(
                "{}: the floor {floor:.2} sits {factor:.2}x below the system's lowest preset \
                 ({lowest_name} at {lowest:.4}), over the {MAX_FLOOR_SLACK:.1}x this file \
                 allows — nothing this system draws comes near the floor, so it would pass \
                 an empty frame the way the pre-ADR-0067 floors did. Re-measure it from the \
                 distribution printed above and say in the commit which preset moved the \
                 minimum",
                system_name(system),
            ));
        }
    }
    slack
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
    let shells = radial_shell_occupancy(&scene, BLACK, EPS);
    println!(
        "[pre-repair ridge] new gate: coverage={cov:.4} (floor {floor:.2}) quadrants={spread} \
         shells={shells}/{RADIAL_SHELLS}"
    );
    assert!(
        cov < floor,
        "a contour drawn 3.3 world units off a frame of half-height 1.0 must FAIL the \
         coverage floor once the vignette stops counting as a figure: coverage {cov:.4} \
         >= {floor:.2}"
    );
    // Catching a scene that renders nothing is the one job the coverage floor
    // demonstrably did, and the structural rescue (Plan 0075 Phase 1) must not
    // reopen it: a figure that is not there occupies no shell, so the rescue
    // cannot reach it.
    assert!(
        shells < MIN_STRUCTURAL_SHELLS,
        "a scene that renders nothing must fail the structural rescue too: \
         {shells}/{RADIAL_SHELLS} shells >= {MIN_STRUCTURAL_SHELLS}"
    );
    assert!(
        old_cov > cov * 10.0,
        "the old gate's score must be dominated by the backdrop, not by the scene: \
         old {old_cov:.4} vs new {cov:.4}"
    );

    // (3) Phase 3's second excitation, on the same fixture. The defect is total
    // rather than level-dependent: the contour is already off frame at MODERATE,
    // which is why the excitation *ratio* cannot see it (0/0) and why
    // MODERATE_MIN_COVERAGE is the check that does.
    let quiet = renderer
        .capture_preset(name, &excited(MODERATE), FRAMES)
        .expect("capture the pre-repair ridge at moderate excitation");
    let mid_cov = coverage(&quiet, BLACK, EPS);
    println!(
        "[pre-repair ridge] at excitation {MODERATE}: coverage={mid_cov:.4} \
         (min {MODERATE_MIN_COVERAGE:.2}), ratio {:.4}",
        ratio_of(cov, mid_cov)
    );
    assert!(
        mid_cov < MODERATE_MIN_COVERAGE,
        "the pre-repair ridge is off frame at a realistic level too, so it must fail the \
         moderate-excitation sentinel: coverage {mid_cov:.4} >= {MODERATE_MIN_COVERAGE:.2}"
    );
}

/// **The three retired ring mandalas at their honest tunings**, recovered from
/// `git show 654304a^:presets/star_mandala.toml` (and siblings) — every
/// `[generator]` table and every binding byte-for-byte, comments stripped and
/// the names suffixed so the output reads clearly. Nothing here is tunable:
/// these are backlog 0072's evidence, frozen.
///
/// "Honest" means what Plan 0065 Phase 5 shipped when it was told not to buy
/// coverage with `glow` and `trails`, and did not: tuned by eye at 1280×720,
/// `glow` at the engine's 1.0, no `trails` binding at all. All three **failed**
/// the 0.34 coverage floor at this test's 96×96 — `0.2442` / `0.2505` /
/// `0.2544`, pinned in the backlog entry — because a hairline over a 46-fold
/// ornament aliases to nothing at that size and `coverage` was measuring the
/// halo. The presets were later retired for an unrelated defect (sampled
/// polylines show their vertices, backlog 0073), which is why they are fixtures
/// here rather than members of the shipped roster.
fn retired_mandalas() -> Vec<Preset> {
    let star_mandala = r#"
system = "star_pattern"
name   = "Star Mandala (retired)"

[generator]
tiling = "none"

rings = [
  { motif = "trefoil",  count = 1,  radius = 0.00, scale = 0.46 },
  { motif = "diamond",  count = 12, radius = 0.30, scale = 0.20 },
  { motif = "petal",    count = 18, radius = 0.52, scale = 0.26, phase = 0.09 },
  { motif = "circle",   count = 24, radius = 0.70, scale = 0.13 },
]

[params]
ring_phase = "time * 0.085 + clamp(bass * 0.22, 0, 0.18)"
ring_spread = "1.00 + sin(time * 0.83) * 0.075 + clamp(bass * 0.13, 0, 0.11)"
ring_scale  = "1.00 + sin(time * 0.61) * 0.115 + clamp(mid * 0.20, 0, 0.17)"
rotation = "0.09 * time"
scale    = "1.06 + sin(time * 0.22) * 0.045"
zoom     = "1.02 + sin(time * 0.15) * 0.035"
thickness  = "3.10 + clamp(bass * 1.60, 0, 1.30) + beat * 0.45"
brightness = "0.95 + clamp(treb * 0.40, 0, 0.34)"
hue        = "0.58 + time * 0.011 + clamp(treb * 0.38, 0, 0.28)"
hue_spread = "0.60 + clamp(mid * 0.22, 0, 0.18)"
saturation = "0.94"
bg_hue      = "0.68 + sin(time * 0.0085) * 0.05"
bg_bright   = "0.016 + clamp(mid * 0.012, 0, 0.010)"
bg_vignette = "0.80"

[smoothing]
ring_phase  = 0.25
ring_spread = 0.35
ring_scale  = 0.40
rotation    = 0.20
scale       = 0.45
zoom        = 0.22
thickness   = { attack = 0.02, release = 0.32 }
hue         = 0.40
hue_spread  = 0.55
bg_bright   = 0.40
"#;
    let mandala_six = r#"
system = "star_pattern"
name   = "Mandala Six (retired)"

[generator]
tiling = "none"

rings = [
  { motif = "trefoil",  count = 1,  radius = 0.00, scale = 0.34 },
  { motif = "circle",   count = 8,  radius = 0.22, scale = 0.143 },
  { motif = "diamond",  count = 14, radius = 0.38, scale = 0.144 },
  { motif = "petal",    count = 20, radius = 0.55, scale = 0.146, phase = 0.08 },
  { motif = "chevron",  count = 28, radius = 0.70, scale = 0.133 },
  { motif = "circle",   count = 36, radius = 0.82, scale = 0.122 },
]

[params]
ring_phase = "time * 0.062 + clamp(bass * 0.17, 0, 0.14)"
ring_spread = "1.00 + sin(time * 0.71) * 0.095 + clamp(bass * 0.15, 0, 0.13)"
ring_scale  = "1.00 + sin(time * 0.49) * 0.100 + clamp(mid * 0.18, 0, 0.15)"
rotation = "-0.06 * time"
scale    = "0.94 + sin(time * 0.19) * 0.040"
zoom     = "1.02 + sin(time * 0.13) * 0.030"
thickness  = "2.70 + clamp(bass * 1.40, 0, 1.15) + beat * 0.40"
brightness = "0.95 + clamp(treb * 0.38, 0, 0.32)"
hue        = "0.12 + time * 0.009 + clamp(treb * 0.34, 0, 0.26)"
hue_spread = "0.78 + clamp(mid * 0.18, 0, 0.15)"
saturation = "0.90"
bg_hue      = "0.10 + sin(time * 0.0070) * 0.04"
bg_bright   = "0.014 + clamp(mid * 0.010, 0, 0.008)"
bg_vignette = "0.82"

[smoothing]
ring_phase  = 0.25
ring_spread = 0.35
ring_scale  = 0.40
rotation    = 0.20
scale       = 0.45
zoom        = 0.22
thickness   = { attack = 0.02, release = 0.32 }
hue         = 0.40
hue_spread  = 0.55
bg_bright   = 0.40
"#;
    let mandala_weave = r#"
system = "star_pattern"
name   = "Mandala Weave (retired)"

[generator]
tiling            = "12"
contact_angle_deg = 26

rings = [
  { motif = "trefoil",  count = 1,  radius = 0.00, scale = 0.46 },
  { motif = "diamond",  count = 12, radius = 0.30, scale = 0.20 },
  { motif = "petal",    count = 18, radius = 0.52, scale = 0.26, phase = 0.09 },
  { motif = "circle",   count = 24, radius = 0.70, scale = 0.13 },
]

[params]
variant = "2 * abs(mod(0.5 + time * 0.012 + clamp(bass * 0.26, 0, 0.22), 2) - 1)"
ring_phase = "time * 0.070 + clamp(bass * 0.20, 0, 0.16)"
ring_spread = "1.00 + sin(time * 0.77) * 0.055 + clamp(bass * 0.10, 0, 0.085)"
ring_scale  = "1.00 + sin(time * 0.53) * 0.090 + clamp(mid * 0.16, 0, 0.14)"
rotation = "0.075 * time"
scale    = "0.90 + sin(time * 0.20) * 0.040"
zoom     = "1.02 + sin(time * 0.14) * 0.030"
thickness  = "3.30 + clamp(bass * 1.55, 0, 1.25) + beat * 0.45"
brightness = "0.95 + clamp(treb * 0.40, 0, 0.34)"
hue        = "0.86 + time * 0.010 + clamp(treb * 0.36, 0, 0.28)"
hue_spread = "0.66 + clamp(mid * 0.20, 0, 0.16)"
saturation = "0.92"
bg_hue      = "0.58 + sin(time * 0.0080) * 0.05"
bg_bright   = "0.016 + clamp(mid * 0.012, 0, 0.010)"
bg_vignette = "0.80"

[smoothing]
variant     = 1.6
ring_phase  = 0.25
ring_spread = 0.35
ring_scale  = 0.40
rotation    = 0.20
scale       = 0.45
zoom        = 0.22
thickness   = { attack = 0.02, release = 0.32 }
hue         = 0.40
hue_spread  = 0.55
bg_bright   = 0.40
"#;
    [star_mandala, mandala_six, mandala_weave]
        .into_iter()
        .map(|toml| Preset::from_toml_str(toml).expect("a retired mandala fixture parses"))
        .collect()
}

/// **Plan 0075 Phase 1's done-when, asserted on the frozen evidence.** The
/// honest tunings that failed the old floor pass the new measure:
///
/// 1. Each retired mandala still **fails** the `star_pattern` coverage floor —
///    the defect is real and unrepaired, so the rescue is doing work rather
///    than decorating. If this half ever fails, the coverage statistic has
///    started seeing thin strokes at 96×96 (a capture-size or measurement
///    change), and [`MIN_STRUCTURAL_SHELLS`] should be re-examined rather than
///    this assertion loosened.
/// 2. Each occupies at least [`MIN_STRUCTURAL_SHELLS`] radial shells, so the
///    full gate would now pass it — with the halo levers at `glow = 1.0` and no
///    `trails`, which is the tuning the old floor rejected and the user asked
///    for.
/// 3. Each still clears the checks the rescue must **not** bypass: quadrant
///    spread and tonal flatness. The rescue answers "blank", not "dot" or
///    "blot".
#[test]
fn the_honest_mandala_tunings_pass_the_structural_measure() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = loud();
    let fixtures: Vec<Preset> = retired_mandalas()
        .into_iter()
        .map(without_backdrop)
        .collect();
    let names: Vec<String> = fixtures.iter().map(|p| p.name.clone()).collect();
    renderer.set_presets(fixtures);
    let floor = coverage_floor(SystemKind::StarPattern);

    for name in &names {
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture a retired mandala fixture");
        let cov = coverage(&img, BLACK, EPS);
        let spread = quadrant_spread(&img, BLACK, EPS);
        let flat = tonal_flatness(&img, BLACK, EPS);
        let shells = radial_shell_occupancy(&img, BLACK, EPS);
        println!(
            "[honest mandala] {name}: coverage={cov:.4} (floor {floor:.2}) \
             shells={shells}/{RADIAL_SHELLS} (min {MIN_STRUCTURAL_SHELLS}) \
             quadrants={spread} flatness={flat:.4}"
        );
        assert!(
            cov < floor,
            "{name} now clears the coverage floor ({cov:.4} >= {floor:.2}) — the defect \
             this rescue exists for has moved; re-derive MIN_STRUCTURAL_SHELLS against \
             whatever changed the measurement"
        );
        assert!(
            shells >= MIN_STRUCTURAL_SHELLS,
            "{name} is the honest tuning the old floor rejected and it must pass the \
             structural measure: {shells}/{RADIAL_SHELLS} shells < {MIN_STRUCTURAL_SHELLS}"
        );
        assert!(
            spread >= MIN_QUADRANTS,
            "{name} must clear the quadrant check the rescue does not bypass: {spread}"
        );
        assert!(
            flat <= MAX_TONAL_FLATNESS,
            "{name} must clear the flatness ceiling the rescue does not bypass: {flat:.4}"
        );
    }
}

/// The least coverage a preset may paint at [`MODERATE`] and still be a picture
/// at a realistic level (Plan 0058 Phase 3). A **sentinel, not a floor** — one
/// number across all eight systems, deliberately unlike [`coverage_floor`].
///
/// The per-system floors were measured at [`LOUD`] and belong there. This asks a
/// cruder question that needs no per-system calibration: *is the figure in the
/// frame at all when the music is merely playing?* Measured the same way
/// regardless — the library's lowest coverage at `MODERATE` is `0.0891`
/// (`Spectrum Ridge`), so `0.04` sits a factor of `2.23` below it, matching
/// [`MAX_FLOOR_SLACK`]'s ceremony.
///
/// **Non-vacuous, and by the case that motivated the plan**: the pre-repair
/// `spectrum_ridge` scores `0.0000` here as well as at `LOUD`, asserted in
/// [`the_pre_repair_ridge_passed_the_old_gate_and_fails_this_one`].
///
/// It is not redundant with the `LOUD` floors, though the overlap is worth being
/// honest about. A figure driven *off* frame fails at `LOUD` and is caught there.
/// What only this can catch is the inverse: a look that is in frame when driven
/// hard and **absent at the level music actually occupies** — a threshold sitting
/// above the material, which is ADR-0062's saturation defect pointed the other
/// way. No shipped preset is in that state, so this is a guard rather than a
/// conviction today. `Rose Draw` is the closest thing to it and is legitimate:
/// `0.1403` at `MODERATE` against `0.9180` at `LOUD`, because at `0.4` the curve
/// is still being drawn.
const MODERATE_MIN_COVERAGE: f32 = 0.04;

/// **Plan 0058 Phase 3 — "more audio must not mean less picture", measured.**
///
/// Captures every preset at [`MODERATE`] and [`LOUD`] and reports
/// `coverage(loud) / coverage(moderate)` — a ratio against the preset's own
/// quieter frame, because the scenes differ by an order of magnitude in how much
/// they paint and an absolute floor could not compare them.
///
/// # This ships as a report, not as a gate, and the measurement is why
///
/// The plan authorized either outcome and left the choice to the numbers
/// (Risks). They came back like this — the whole library, plus the pre-repair
/// ridge as a control:
///
/// ```text
///  ratio   cov@0.4  cov@1.0  preset
///  0.8552   0.2878   0.2461  De Jong          <- lowest legitimate
///  0.9568   0.3164   0.3027  Leviathan
///  0.9935   1.0000   0.9935  Warp Drive
///  ...      (25 presets between 0.99 and 1.11)
///  1.0514   0.3866   0.4065  Spectrum Corona  <- over-scaled, scale = 5.20
///  1.0891   0.5088   0.5541  Spectrum Comb    <- over-scaled, scale = 3.80
///  1.3350   0.0891   0.1189  Spectrum Ridge   (repaired)
///  1.9753   0.4047   0.7995  Star Rosette
///  6.5429   0.1403   0.9180  Rose Draw        <- highest
///     inf   0.0000   0.0000  Spectrum Ridge (pre-repair)
/// ```
///
/// **No threshold on this axis convicts anything it was built for.** Three known
/// defective configurations exist and the ratio reaches none of them:
///
/// - **`Spectrum Comb` does not fail. It scores `1.0891` — it draws *more* when
///   loud.** The plan named it the live candidate and asked for this stated
///   plainly if it came back clean, so: a comb roots every bar on a shared
///   baseline, and clipping the tips off the tallest bars costs a rounding error
///   of coverage while the body of the figure stays exactly where it was. The
///   layout the check was designed around is the layout it cannot see.
/// - **`Spectrum Corona` is the same at `1.0514`**, for the same reason.
/// - **The pre-repair ridge is `0/0`, undefined.** Its contour is already off
///   frame at `MODERATE` (`scale = 3.20` puts a driven element ~1.9 world units
///   up at level `0.4`, against a half-height of `1.0`), so there is no
///   moderate-excitation picture to compare the loud one against. A ratio needs a
///   denominator and a total defect does not have one.
///
/// Meanwhile the only content anywhere near a plausible threshold is **correct**:
/// `De Jong` at `0.8552` and `Leviathan` at `0.9568` are the attractor family's
/// deliberate *peak buys structure* idiom, which ADR-0062's Alternatives records
/// as real and as the reason a directional assertion was rejected there too. A
/// gate at `0.80` would sit `0.055` from `De Jong` — tight enough that a retune
/// trips it — while catching none of the three cases above. That trade is
/// strictly negative: every unit of sensitivity buys risk of convicting the
/// attractor idiom and zero demonstrated detection.
///
/// So the ratio is printed, watched, and not enforced. What *is* enforced here is
/// [`MODERATE_MIN_COVERAGE`], the one property the second capture supports: a
/// preset must be a picture at a realistic level, not only when fully driven.
///
/// **What this means for the class the plan was aiming at.** The over-scale
/// defect is real and this instrument does not reach it — pixel coverage is the
/// wrong measure for a figure whose *tips* leave the frame, because tips are
/// almost no pixels. ADR-0067 already names the successor: an in-frame **geometry
/// fraction**, rejected there as the primary mechanism but explicitly kept as the
/// supplement for the line and spectrum families. This measurement is the
/// evidence that it is now wanted.
#[test]
fn a_louder_frame_is_reported_against_a_quieter_one() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let (presets, meta) = sanity_roster();
    renderer.set_presets(presets);
    let (mid_frame, loud_frame) = (excited(MODERATE), excited(LOUD));

    let mut rows: Vec<(f32, f32, f32, String)> = Vec::new();
    let mut failures = Vec::new();
    for (name, _system) in &meta {
        let mid_cov = coverage(
            &renderer
                .capture_preset(name, &mid_frame, FRAMES)
                .expect("capture at moderate excitation"),
            BLACK,
            EPS,
        );
        let loud_cov = coverage(
            &renderer
                .capture_preset(name, &loud_frame, FRAMES)
                .expect("capture at loud excitation"),
            BLACK,
            EPS,
        );
        rows.push((ratio_of(loud_cov, mid_cov), mid_cov, loud_cov, name.clone()));

        if mid_cov < MODERATE_MIN_COVERAGE {
            failures.push(format!(
                "{name} is not a picture at a realistic level: coverage {mid_cov:.4} at \
                 excitation {MODERATE} is under {MODERATE_MIN_COVERAGE:.2} (it draws \
                 {loud_cov:.4} at {LOUD}). Either a threshold in this preset sits above the \
                 level music occupies, or a world-space param has already carried the figure \
                 out of frame by {MODERATE}"
            ));
        }
    }

    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!(
        "excitation ratio — coverage at {LOUD} over coverage at {MODERATE} (a report, not a \
         gate; this test's doc comment says why):"
    );
    println!("     ratio  cov@{MODERATE}  cov@{LOUD}  preset");
    for (ratio, mid_cov, loud_cov, name) in &rows {
        println!("  {ratio:>8.4}   {mid_cov:.4}   {loud_cov:.4}  {name}");
    }

    assert!(
        failures.is_empty(),
        "these presets draw nothing at a realistic level: {failures:#?}"
    );
}

/// `loud / moderate`, or infinity when the preset painted nothing at moderate —
/// a total defect has no denominator, which is itself a finding rather than a
/// division to guard against.
fn ratio_of(loud_cov: f32, mid_cov: f32) -> f32 {
    if mid_cov > 0.0 {
        loud_cov / mid_cov
    } else {
        f32::INFINITY
    }
}
