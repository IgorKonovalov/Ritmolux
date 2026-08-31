//! Shape sanity (Plan 0013 Phase 3, HARD). A newly-added scene that drew nothing
//! or a single dot should fail before it ships. Under a sustained *loud* frame
//! (so audio-gated brightness is up), assert each preset lights a minimum
//! fraction of the frame (`coverage`) and spreads across at least two quadrants
//! (`quadrant_spread`) — "not blank, not a dot".
//!
//! # The reference is neither the frame's corner nor a fixed black
//!
//! Both are traps, and each one has already let a defect ship.
//!
//! **Not the corner** (ADR-0067). `bg_vignette` darkens the frame toward its
//! edges, so on any preset that binds one **pixel (0, 0) is the darkest pixel in
//! the image** and nearly every pixel toward the centre differs from it by more
//! than [`EPS`] — the backdrop itself reads as a large, well-spread, lit figure,
//! satisfying the sparse floor whatever the scene does. So the roster this gate
//! renders has its `bg_*` bindings **removed** ([`without_backdrop`]). The
//! background stage already defaults `bright` and `vignette` to `0.0`
//! (`core/src/render/background.rs`), so this is *not applying three bindings*
//! rather than a new render path. Nothing outside this file changes — `golden`,
//! `distinctness`, `reactivity` and `shot` all keep the shipped composite.
//!
//! **Not a fixed black** (ADR-0126). A scene that paints its **own** ground is
//! measured against nothing there: the paper is not black, so every pixel counts
//! as lit and the frame reads exactly `1.0000` however little was drawn on it —
//! which makes an emptied canvas and a broken one the same picture
//! ([`a_canvas_the_music_empties_is_convicted_and_black_calls_it_full`]). So
//! [`ground`] estimates the reference per capture, as the mean tone of the
//! frame's most populous luminance band, and every statistic below asks the same
//! question in both worlds: *how far does this picture depart from the ground it
//! is drawn on?* A scene drawing light onto darkness is unaffected, its modal
//! band **being** the black it was already compared to. [`BLACK`] survives as
//! the reference the two-lens fixtures assert against, not as anything the gate
//! reads.
//!
//! # The floors are per-system, and there are three questions
//!
//! Coverage floors stay per-system because the systems differ by an order of
//! magnitude in how much they paint — `fragment_field` fills the frame while
//! `spectrum` draws a contour — so a single broad floor would be either
//! tautological for one or impossible for the other. They sit at half each
//! system's lowest shipped preset and [`MAX_FLOOR_SLACK`] keeps them there. The
//! `swarm` is not sparse points: it measures `0.84`.
//!
//! **A second excitation** (Plan 0058 Phase 3), because one fully-driven frame
//! cannot see a figure that is fine at rehearsal level and gone at the top of
//! its range. [`a_louder_frame_is_reported_against_a_quieter_one`] captures at
//! [`MODERATE`] as well as [`LOUD`] and reports the ratio — a **report, not a
//! gate**, since no threshold on that axis convicts any known defective
//! configuration while the nearest content to one is the attractor family's
//! deliberate idiom. The second capture buys one gate,
//! [`MODERATE_MIN_COVERAGE`]: that a preset is a picture at a realistic level.
//!
//! **A third question — does the shape have an interior?** "Not blank, not a
//! dot" is satisfied completely by a fully saturated single-tone mass, which is
//! a real figure, the right size, in every quadrant, and a blot. `tonal_flatness`
//! is the statistic that names it, and it is general rather than
//! attractor-specific: any drive that stacks past the additive ceiling produces
//! it. It is also the one statistic the derived ground **cannot** repair — a
//! duotone has two large populations, so removing whichever is the ground leaves
//! the other holding nearly all of what remains either way (ADR-0128).

use lmv_core::{
    dsp::AnalysisFrame,
    preset::{Preset, SystemKind, default_presets},
    render::{
        CaptureImage,
        metrics::{
            RADIAL_SHELLS, TONE_BANDS, boundary_density, coverage, modal_ground, quadrant_spread,
            radial_shell_occupancy, tonal_flatness,
        },
    },
};

mod common;

const SIZE: u32 = 96;
const FRAMES: u32 = 30;
/// A pixel counts as lit if any RGB channel differs from the frame's [`ground`]
/// by more than this (shrugs off dithering at the ground's own tone).
const EPS: u8 = 10;
/// What the scene was measured against between ADR-0067 and Plan 0116 Phase 3 —
/// **the historical reference, not what the gate reads.** [`ground`] is what it
/// reads now.
///
/// It is kept because two fixtures here assert against *both* lenses, which is
/// the only way a test can show that the change repaired something rather than
/// merely moved a number: `a_frame_with_no_tonal_structure_is_reported_flat`
/// freezes the demonstration that a blot clears every areal check against black,
/// and `a_canvas_the_music_empties_is_convicted_and_black_calls_it_full` pins
/// that a painted ground reads as a completely full frame against it. Alpha is
/// never compared — `metrics::is_lit` takes the first three channels — but the
/// frames come back opaque, so 255 is the honest value.
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// The reference tone this gate hands `is_lit`, derived from the frame
/// (**Plan 0116 Phase 3**, ADR-0126) rather than fixed at [`BLACK`].
///
/// Every statistic below asks *how far does this pixel depart from the ground*,
/// and a constant reference answers that question only in a world where the
/// scene draws light onto a ground it does not own. Twelve shipped presets
/// paint their own — the attractor's ink duotones, every `fragment_field`
/// preset — and read `coverage` exactly `1.0000` whatever they drew, which
/// makes three of the four statistics constants rather than measurements for
/// that content.
///
/// **It is the same lens in the world it was built for.** Plan 0116 Phase 1
/// tabled this estimator against the whole library at both excitations: it
/// re-bases 17 of 41 presets and moves **no verdict**, at either excitation.
/// A dark-ground scene's modal band *is* the black it was already measured
/// against, so the substitution is a no-op there and a repair everywhere else.
fn ground(img: &CaptureImage) -> [u8; 4] {
    modal_ground(img)
}

/// The prefix every background-stage parameter carries (`bg_hue`, `bg_bright`,
/// `bg_vignette` — `core/src/render/background.rs`'s `PARAMS`, which is
/// `pub(crate)` and so not nameable from an integration test).
/// [`sanity_roster`] asserts the prefix still matches something, so a rename
/// fails this gate rather than silently restoring the backdrop.
const BG_PREFIX: &str = "bg_";
/// Minimum lit quadrants — a dot in one corner fails.
const MIN_QUADRANTS: u8 = 2;

/// Maximum share of the lit figure that may sit inside one narrow luminance
/// band — the point past which the picture has no tonal structure left, only a
/// mass of one tone.
///
/// # It is one of two terms, not a verdict
///
/// Crossing this ceiling convicts nothing on its own. A preset is failed only
/// when it is **also** below [`boundary_floor`] on `metrics::boundary_density` —
/// whether the lit set has any interior, the orthogonal question ADR-0128 asked
/// for and ADR-0130 settled. Read every measurement below as *the first half of
/// a conjunction*: the distribution, the `0.0161` margin and the shelf life are
/// all still true of this statistic, and none of them is a verdict.
///
/// The reason the second term is needed rather than a wider ceiling: a two-ink
/// print reads near `1.0` here because being tonally flat is what a two-ink
/// print *is*, and no ground estimator repairs it (all three were measured).
///
/// # Where the number came from
///
/// `coverage` and `quadrant_spread` ask *is something there* and *is it more
/// than a dot*, and a fully saturated single-tone mass answers yes to both: it
/// is a real shape, the right size, in every quadrant, and it is also a blot.
/// This is the third question.
///
/// **Measured from the shipped library's own values** under the ADR-0067 capture
/// (backdrop suppressed, compared against [`BLACK`]).
/// `every_preset_draws_a_real_shape` prints the whole distribution on every run;
/// its top:
///
/// ```text
/// 0.8839   Rose Web
/// 0.7211   Rose Trails
/// 0.6755   Ink on Paper
/// 0.4249   Cathedral
/// 0.4107   Aurora
/// 0.3604   Supernova
/// 0.3147   Leviathan
/// 0.3014   Rose Overflow
/// ```
///
/// **The margin above the library is `0.0161`** (`0.90` over `Rose Web`), and
/// the deliberately flattened fixture reads `0.9815` — so `0.90` separates the
/// library from the fixture but sits `0.0161` above one and `0.0915` below the
/// other, **not a midpoint**. It is left there because the margin narrowed for a
/// real reason: the top of the distribution is an actual figure with an actual
/// tonal problem, and a preset drifting over the ceiling is a preset to route,
/// not a constant to nudge.
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
/// the preset. See design-backlog 0053: neither `coverage` nor `quadrant_spread`
/// can distinguish a vignette from a figure, so this statistic convicted the
/// right preset for the wrong reason.
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
///
/// **Re-checked against the derived ground on 2026-08-26 (Plan 0116 Phase 4) and
/// held.** Deriving the reference per capture lowers many measured coverages —
/// fourteen presets came off `1.0000` — which moves this factor *down*, the
/// direction that tightens floors rather than loosening them. The five floors
/// re-derived below sit at `1.95x`-`2.06x`; the six left alone sit at
/// `0.28x`-`1.78x`. Nothing approaches the cap, so the constant is unchanged.
///
/// **What that re-check exposed, which this constant cannot see.** It is
/// one-sided: it fires when a floor sits too far *below* its family, and says
/// nothing when a floor sits *above* it. Six families are in that state today —
/// `parametric_curve`, `emitter`, `star_pattern`, `spectrum`,
/// `reaction_diffusion` and `shape_field` — and their thinnest members clear the
/// gate through the structural rescue rather than the floor. That predates Plan
/// 0116 (their coverages are byte-identical under both lenses) and is not this
/// file's to decide unilaterally, because the rescue carrying a thin figure is
/// the design of Plan 0075, not a defect. It is raised in Plan 0116's log.
const MAX_FLOOR_SLACK: f32 = 2.2;

/// Per-system minimum lit fraction, **measured from the shipped library** under
/// the ADR-0067 capture (backdrop suppressed) against the frame's own derived
/// ground (ADR-0126).
///
/// **Each floor is half its system's lowest shipped preset**, so the gap is a
/// factor of ~2 everywhere and [`MAX_FLOOR_SLACK`] holds it there. Five floors
/// were re-derived on 2026-08-26 and six were not, and the split is measured
/// rather than chosen: the derived ground only moves a preset that paints its
/// own, so for a scene drawing light onto darkness the modal band *is* the black
/// it was already compared to and the coverage comes back identical to the digit.
///
/// ```text
/// system              floor          family minimum              why
/// fragment_field      0.50 -> 0.08   1.0000 -> 0.1645 Tiled Rosette   all eight came off 1.0000
/// lsystem             0.50 -> 0.19   1.0000 -> 0.3704 Vellum          the only member
/// swarm               0.28 -> 0.33   0.5553 -> 0.6531 Shatter         spin widened
/// attractor           0.18 -> 0.11   0.2214 -> 0.2156 De Jong Gallery marginal
/// shape_field         0.50 -> 0.22   0.4312 Pulse (unmoved)           the arm's text was false
/// parametric_curve    0.33           0.2273 Ion Wake (unmoved)        lit-on-dark
/// emitter             0.25           0.0696 Ember Jet (unmoved)       lit-on-dark
/// star_pattern        0.34           0.1484 Rose Window (unmoved)     lit-on-dark
/// spectrum            0.28           0.2604 Halo (unmoved)            lit-on-dark
/// reaction_diffusion  0.09           0.1603 Mitosis (unmoved)         lit-on-dark
/// ```
///
/// **The table is a snapshot; the distribution `every_preset_draws_a_real_shape`
/// prints on every run is authoritative.** Upward drift in a family minimum is
/// the safe direction and [`MAX_FLOOR_SLACK`] is what eventually calls it.
///
/// # Two traps for whoever re-measures
///
/// **`ink_*` is a terminal engine stage rather than a `bg_*` binding, so no
/// backdrop suppression reaches it.** This is the only place in the tree that
/// records it, and any future lens assuming a suppressed frame is dark will be
/// wrong here for the same reason the [`BLACK`] predicate was: a preset setting
/// `ink_amount = 1` renders paper-white, every pixel differs from black, and the
/// frame reads `1.0000` as a measurement artifact rather than a saturated figure.
///
/// **A `1.0000` in any older record is a [`BLACK`]-predicate reading, not a
/// current one.** Against the derived ground the two presets that produced them
/// read `0.2167` and `0.2917`, which is what the `SystemKind::Attractor` arm
/// below is half of. Quoting a historical reading as current is the mistake.
///
/// # What the factor of 2 costs, stated rather than discovered
///
/// This is deliberately the sensitive end of the range, unlike
/// [`SATURATED_OCCUPANCY`](lmv_core::preset::SATURATED_OCCUPANCY), which took a
/// wide margin because a HARD gate firing on good content buys exemptions. The
/// difference is what "wrong" looks like on each side: an over-driven clamp is a
/// *number* that stopped moving and a generous threshold still catches it,
/// whereas an off-frame figure is a *picture that is not there*. A new preset
/// failing one of these has drawn less than half of what the thinnest shipped
/// member of its own family draws, which is worth a look even when it is fine.
///
/// One caveat on the statistic itself: at this test's 96x96 capture a hairline
/// over a dense ornament aliases to almost nothing, so `coverage` there measures
/// the halo and the trail rather than the figure — a bare rosette and a 46x-denser
/// mandala scored *identically*. design-backlog 0072 stays open on it and asks
/// for a structural occupancy measure.
///
/// The families that vary internally by 3-5x sit over the most movement and are
/// the ones most likely to need a re-measure. The response to a legitimately
/// sparser new preset is to re-derive that system's floor from the printed
/// distribution, and to say in the commit which preset moved the minimum — not
/// to nudge a constant back until the run goes green.
fn coverage_floor(system: SystemKind) -> f32 {
    match system {
        // Re-derived 2026-08-26 (Plan 0116 Phase 4) from 0.50. The old number
        // came from a family where all eight members read 1.0000 and the spread
        // was 0.0074 wide — which was the degeneracy ADR-0126 was raised on,
        // not a measurement: `coverage` was reading the paper these scenes
        // paint. Against their own ground the family spreads 0.1645-0.9969 and
        // `Tiled Rosette` sets the minimum, so this is half of that. It is a
        // large drop and it costs nothing the shells do not already catch: a
        // broken field scores near zero at zero shells and is convicted, while
        // `Tiled Rosette` was already riding the structural rescue at 9/10.
        SystemKind::FragmentField => 0.08,
        // A dense point cloud that fills the frame far more than "sparse points"
        // suggested — the old 0.01 was 84x below the thinnest of the three.
        // Re-derived 2026-08-27 (Plan 0122 Phase 5) from 0.28, which the slack
        // gate convicted at 2.33x: `Shatter` still sets the family minimum and
        // it MOVED, 0.5553 -> 0.6531, when its `spin` swing widened from 0.75 to
        // 2.2. That widening is the content half of the rate correction — `spin`
        // integrates a phase now, so the range is an honest rate rather than a
        // multiplier on elapsed time — and a faster-churning field spreads the
        // cloud. Half the new minimum, as every floor here is. The family is two
        // presets and both bind `spin`, so there is no unaffected member to hold
        // this number still: expect to re-derive it whenever either is retuned.
        SystemKind::Swarm => 0.33,
        // Line art. The trails-heavy looks score lowest because a faint tail is
        // still lit; Rose Trails at 0.6722 sets this one.
        SystemKind::ParametricCurve => 0.33,
        // Raised from 0.32 on 2026-08-13 when `Wildwood` was retired on sight in
        // the running app: it was the family minimum, and its removal left
        // `Vellum` at 1.0000 as the only shipped member, putting the old floor
        // 3.12x below it — over this file's 2.2x slack.
        //
        // Re-derived 2026-08-26 (Plan 0116 Phase 4) from 0.50. `Vellum`'s
        // 1.0000 was the same paper artifact as the `fragment_field` eight —
        // against its own ground it draws 0.3704 — so this is half of that. A
        // one-member family means the next lsystem preset will very likely move
        // this number again.
        SystemKind::LSystem => 0.19,
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
        //
        // Re-derived 2026-08-26 (Plan 0116 Phase 4) from 0.18. The minimum
        // barely moved (`De Jong Gallery` 0.2214 -> 0.2156), but the family's
        // *ink duotones* did: `Ink on Paper`, `Thomas` and `Valentine` came off
        // 1.0000 to 0.2167 / 0.2917 / 0.4389, which is the artifact the note
        // below this table describes, and it is not a live one. Half the new
        // minimum.
        SystemKind::Attractor => 0.11,
        // The sparsest system in the library, and the one Plan 0058 exists
        // because of. Spectrum Ridge sets it at 0.1189 — *after* its repair; the
        // version that shipped broken scores 0.0000 here. Raised from 0.06 when
        // cohort four (Plan 0075) retired the three spectrum presets: the family
        // minimum moved up to Halo at 0.5843 (its lit violet atmosphere and
        // thick spokes cover what the old thin combs did not), and the old floor
        // sat 9.7x below it.
        SystemKind::Spectrum => 0.28,
        // A shower of small marks over an otherwise empty frame — sparse by
        // idiom, like the spectrum readout and unlike the swarm's dense cloud.
        // Measured from `Sparks`, the family's only shipped member (Plan 0052),
        // and set at half of it like every floor above. When a second emitter
        // preset lands this is the number to re-derive from the distribution
        // this test prints.
        SystemKind::Emitter => 0.25,
        // **Derived from the distribution for the first time on 2026-08-26**
        // (Plan 0116 Phase 4), from 0.50. Until then this arm said the family
        // "has zero shipped members and this floor has never gated anything",
        // inheriting `FragmentField`'s number on the structural argument that a
        // fullscreen `occlude` scene cannot score low. That was true when Plan
        // 0091 shipped the engine with no content; `Facet` and `Pulse` ship now,
        // and `Pulse` has been under the old floor at 0.4312 — riding the
        // structural rescue — under both lenses. Half of that minimum. (The
        // structural argument is separately dead: `Facet` read 1.0000 only
        // because `coverage` was measuring the ground it paints, and reads
        // 0.5940 against that ground.)
        SystemKind::ShapeField => 0.22,
        // **Not derived from a distribution either**: Plan 0100 ships the
        // `warp_mesh` engine and no preset content, exactly as Plan 0091 shipped
        // `shape_field`. Inherited from `FragmentField` on the same structural
        // argument — the warp mesh presents a fullscreen field with `occlude`,
        // so one that is not broken cannot score low. **Re-derive it from this
        // test's printed distribution when the first one ships**, at half the
        // family minimum like every floor above.
        //
        // **Left at 0.50 on 2026-08-26 while `FragmentField` went to 0.08, so it
        // inherits nothing** (Plan 0116 Phase 4). Two reasons, both recorded
        // rather than resolved: the structural argument it rests on is the one
        // Phase 3 falsified — a fullscreen field scores 1.0 because `coverage`
        // was reading the ground it paints — and the number is duplicated in
        // `core/tests/warp_mesh.rs`, which that phase is not scoped to touch and
        // which asserts the two match. The first `warp_mesh` preset re-derives
        // both together.
        SystemKind::WarpMesh => 0.50,
        // **Derived from the distribution on 2026-08-26** (Plan 0113 Phase 6b),
        // from an inherited `0.50`. Until then this arm read that a
        // `shape_collage` canvas paints its own paper across every pixel
        // (ADR-0123) so "its lit fraction is 1.0 by construction whatever the
        // elements do", and leaned on `MAX_TONAL_FLATNESS` as the rescue. Plan
        // 0116 falsified the premise: `1.0000` was `coverage` measuring the
        // paper against black, and against the ground the frame is actually
        // drawn on the two shipped canvases read `On White` 0.2677 and
        // `Suprematist` 0.3028 — both *under* the inherited floor, clearing the
        // gate only on the structural rescue. Half the new minimum, like every
        // floor above. The areal question is now the one that convicts an
        // emptied canvas, at the quiet excitation where emptying happens —
        // [`a_canvas_the_music_empties_is_convicted_and_black_calls_it_full`].
        SystemKind::ShapeCollage => 0.13,
    }
}

/// Per-system minimum [`boundary_density`] — the **second term** of the flatness
/// conjunction, below which a frame that is also over [`MAX_TONAL_FLATNESS`] is
/// convicted as a blot (ADR-0130).
///
/// # Per system, which is a measurement and not a convenience
///
/// A single global number is provably impossible on this library. The floor must
/// sit **above** the frozen `Blown Out` blot's `0.2631` to convict it, and
/// **below** `Suprematist`'s `0.2565` to admit a mono conversion of an ADR-0123
/// flat-graphic composition. `Suprematist` scores *under the purpose-built
/// defect*, so no such number exists and the split is forced. The mechanism is
/// the one [`coverage_floor`] already uses in this file.
///
/// # The two arms are different kinds of number, and say so (ADR-0071)
///
/// **`0.31` is a measurement between two frozen frames, not a derived floor.**
/// It is the midpoint of `0.2631` (the frozen [`blown_out`] fixture) and
/// `0.3602` (`Tiled Rosette Mono`, measured 2026-08-26 at `8389f2a`), rounded to
/// two places — `1.18x` above the defect and `1.16x` below the composition.
///
/// It is **not** *half the sparsest legitimate content*, the ceremony every
/// `coverage_floor` arm above follows, and the reason is structural: a
/// conjunction's second term is only ever asked about frames that already failed
/// the first, and conditioned that way the population has **two members** — the
/// blot and the one preset this arm exists to admit. Half-the-sparsest would be
/// derived from the very frame being admitted, which is circular (ADR-0129). So
/// the arm claims a two-point measurement and carries its risk in the open:
/// **the first genuinely flat preset from a third family is what tests this
/// number, and it does not exist yet.**
///
/// **`0.13` is the ordinary ceremony** — half the sparsest legitimate member of
/// the family (`Suprematist`, `0.2565`). It leaves `Suprematist` at `1.97x` and
/// `On White` at `2.36x` above their own floor, so both survive a mono
/// conversion with room.
///
/// # Why `shape_collage` earns an arm where an idiom flag would not
///
/// ADR-0123 holds that family's whole canvas under ADR-0046's tonemap knee,
/// giving up the engine's entire over-range vocabulary — no bloom, no glow, no
/// highlight modelling. **The additive stacking that produced `Blown Out` cannot
/// occur there**, so the family is structurally exempt from the defect and
/// therefore from the term that catches it. A `SystemKind` selects a different
/// renderer, where a param is a one-line edit any preset can make — which is
/// what separates this from the idiom scoping ADR-0130 rejected. A
/// `shape_collage` blot would not be caught at `0.13`; if that family ever gains
/// an over-range path, this arm is re-opened with it.
///
/// # Every other family is un-derived on purpose
///
/// The remaining arms take `0.31` because they have no conditional population to
/// derive from: nothing in them is over the flatness ceiling, so the second term
/// has never been asked about them. **22 of the 42 shipped presets read below
/// their family's floor today and pass only because term one clears them** —
/// `every_preset_draws_a_real_shape` prints that count on every run. Converting
/// one to a two-ink print raises its flatness toward `1.0` and leaves its
/// boundary alone, so **each such conversion needs its own arm with its own
/// derivation** before it can ship. For `attractor` that is a real fork: the
/// ceremony-derived number there is `0.0220`, `12x` below the blot and vacuous.
fn boundary_floor(system: SystemKind) -> f32 {
    match system {
        // Half the sparsest legitimate member of the family (`Suprematist`,
        // 0.2565) — this file's own ceremony, on the two shipped members. The
        // family has no over-range path at all (ADR-0123 under ADR-0046's knee),
        // so the additive stack this term exists to catch cannot happen in it.
        SystemKind::ShapeCollage => 0.13,
        // The midpoint of the two frozen fixtures, 0.2631 and 0.3602. A
        // measurement between two frames, NOT half the sparsest legitimate
        // content — the conditional population has one legitimate member and it
        // is the preset being admitted. See the doc comment.
        _ => 0.31,
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
        SystemKind::WarpMesh => "warp_mesh",
        SystemKind::ShapeCollage => "shape_collage",
    }
}

/// A sustained frame with every level driven to `level` and a beat, so any
/// audio-gated brightness reaches its lit state.
///
/// "Every band up" includes the `spectrum` array itself (Plan 0034 Phase 2). A
/// frame with `bass = mid = treb = 1.0` and 64 silent log-bands is not a frame
/// any audio could produce, and under it a spectrum readout would correctly draw
/// almost nothing — the floor would be measuring the fixture, not the scene.
/// Filling it moves only what a `spectrum`-reading scene draws; every other
/// preset's capture is untouched by it.
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

/// One family's shape gate, as each generated `#[test]` calls it.
///
/// **This sweep splits per family and never per preset** (ADR-0157). Its
/// per-preset checks would decompose, but [`report_coverage_distribution`] would
/// not: that gate fails a family whose coverage floor has been left behind by
/// its own lowest preset, which is a claim about the family's *distribution* and
/// has no single-preset form. Splitting per preset would either drop it or
/// re-render the roster a second time to keep it, so the unit here is the family
/// — which still takes the sweep off the critical path, the largest family being
/// 19 presets rather than the whole library.
///
/// The three roster-wide reports become family-wide for the same reason: the
/// coverage distribution is per family already, and the flattest-preset ranking
/// and the under-boundary-floor count are now that family's.
fn draws_a_real_shape(family: &str) {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let frame = loud();
    let (presets, all) = sanity_roster();
    renderer.set_presets(presets);

    let meta: Vec<(String, SystemKind)> = all
        .into_iter()
        .filter(|(_, system)| system_name(*system) == family)
        .collect();
    assert!(
        !meta.is_empty(),
        "no shipped preset declares system `{family}`, so this test measures an empty set and \
         would pass whatever the family's content did. The generated tests come from the same \
         `presets/*.toml` glob as the embedded set, so an empty family here means a preset's \
         `system` key parses to something other than what the build script scanned out of it"
    );

    let mut failures = Vec::new();
    let mut flatness = Vec::new();
    let mut under_boundary_floor: Vec<String> = Vec::new();
    let mut by_system: Vec<(SystemKind, f32, String)> = Vec::new();
    for (name, system) in &meta {
        let (name, system) = (name.as_str(), *system);
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture preset");
        let bg = ground(&img);
        let cov = coverage(&img, bg, EPS);
        let spread = quadrant_spread(&img, bg, EPS);
        let flat = tonal_flatness(&img, bg, EPS);
        let boundary = boundary_density(&img, bg, EPS);
        let shells = radial_shell_occupancy(&img, bg, EPS);
        let floor = coverage_floor(system);
        let b_floor = boundary_floor(system);
        println!(
            "[{}] {name:<12} coverage={cov:.4} (floor {floor:.2}) quadrants={spread} \
             flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2}) \
             boundary={boundary:.4} (floor {b_floor:.2}) shells={shells}/{RADIAL_SHELLS}",
            system_name(system),
        );
        let known_flat = KNOWN_FLAT.contains(&name);
        flatness.push((flat, name.to_string(), known_flat));
        by_system.push((system, cov, name.to_string()));
        if boundary < b_floor {
            under_boundary_floor.push(format!("{boundary:.4} {name}"));
        }
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
        // **The conviction is a conjunction** (Plan 0119 Phase 3, ADR-0130).
        // Neither term is a verdict on its own. A two-ink print reads near 1.0
        // on the tonal term because being flat is what that idiom *is*, and 22
        // of the shipped presets read under their family's boundary floor and
        // are legitimate. Only a frame that is both — one tone AND no interior
        // — is a blot, and only that frame is failed here.
        let convicted = flat > MAX_TONAL_FLATNESS && boundary < b_floor;
        if convicted && !known_flat {
            failures.push(format!(
                "{name} is a blot: {:.1}% of its lit pixels sit in one of {TONE_BANDS} \
                 luminance bands (max {:.0}%) AND its lit set has almost no interior \
                 (boundary {boundary:.4}, the share of lit pixels touching an unlit \
                 neighbour, under the {b_floor:.2} floor for {}). **Both** terms fired, \
                 which is what separates a blot from a flat graphic. The structural term \
                 is the one to answer: the figure is a solid mass, and what it lacks is \
                 perimeter per unit lit area — open its interior with hatching, contours, \
                 tiling or separate marks, rather than spreading the same mass wider. \
                 Lowering the drive, the glow or the accumulation answers the tonal term \
                 only",
                flat * 100.0,
                MAX_TONAL_FLATNESS * 100.0,
                system_name(system),
            ));
        }
        // The list must not outlive the defect. A repaired preset that is still
        // named here would silently exempt whatever it becomes next. It is the
        // *conjunction* that has to still convict it, for the same reason the
        // gate above is one: a preset that only crosses the tonal ceiling is no
        // longer a defect this file names.
        if known_flat && !convicted {
            failures.push(format!(
                "{name} is listed in KNOWN_FLAT but the conjunction no longer convicts it \
                 (flatness {flat:.4} against {MAX_TONAL_FLATNESS:.2}, boundary \
                 {boundary:.4} against {b_floor:.2}) — it was repaired, so delete the entry"
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

    // **The exposure, printed rather than left to be rediscovered** (ADR-0130's
    // landmine Negative). These presets are held out of conviction by term one
    // alone: they read under their own family's boundary floor, and a mono
    // conversion — which raises tonal_flatness toward 1.0 and leaves boundary
    // untouched — is what would put them over both terms at once.
    //
    // **It is a report and never an assertion.** There is no measured basis for
    // a threshold on this count, and the count is *expected* to move as the mono
    // cohort lands. A number here that could redden CI would be a constant
    // nobody derived.
    under_boundary_floor.sort();
    println!(
        "{} of {} presets read under their family's boundary floor and pass on term one \
         alone (reported, not gated):",
        under_boundary_floor.len(),
        meta.len(),
    );
    println!("  {}", under_boundary_floor.join("  |  "));

    assert!(
        failures.is_empty(),
        "these {family} presets failed shape sanity: {failures:#?}"
    );
}

// One `#[test]` per family the shipped set contains, generated by
// `core/build.rs` (ADR-0157).
include!(concat!(env!("OUT_DIR"), "/sanity_shape_tests.rs"));

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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.set_presets(vec![without_backdrop(blown_out())]);
    let img = renderer
        .capture_preset("Blown Out", &loud(), FRAMES)
        .expect("capture the flat fixture");

    let floor = coverage_floor(SystemKind::ParametricCurve);

    // (1) The purely areal lens, without `MAX_TONAL_FLATNESS`, on the same
    // frozen fixture. This is the demonstration that lens was added for and
    // it is kept rather than described: against a constant black reference the
    // blot passes every areal check — full coverage, four quadrants, every
    // radial shell — and only the tonal question convicts it.
    let old_cov = coverage(&img, BLACK, EPS);
    let old_spread = quadrant_spread(&img, BLACK, EPS);
    let old_shells = radial_shell_occupancy(&img, BLACK, EPS);
    let old_flat = tonal_flatness(&img, BLACK, EPS);
    println!(
        "[blown out] against BLACK: coverage={old_cov:.4} (floor {floor:.2}) \
         quadrants={old_spread} shells={old_shells}/{RADIAL_SHELLS} flatness={old_flat:.4}"
    );
    assert!(
        old_cov >= floor && old_spread >= MIN_QUADRANTS && old_shells >= MIN_STRUCTURAL_SHELLS,
        "the fixture must clear every areal check against black, or it proves nothing about \
         why the tonal question was added: coverage {old_cov:.4} (floor {floor:.2}), \
         {old_spread} quadrant(s), {old_shells}/{RADIAL_SHELLS} shells"
    );
    assert!(
        old_flat > MAX_TONAL_FLATNESS,
        "a figure stacked past the additive ceiling must read flat, got {old_flat:.4}"
    );

    // (2) The lens as it stands now. A blot that fills the frame with one tone
    // **is its own modal band**, so the derived ground lands on the blot itself
    // and the lit mask is what is left over — the figure's fringe. The fixture
    // is therefore convicted twice rather than once, which is a stronger
    // verdict and a weaker demonstration: coverage does not score it healthy.
    let bg = ground(&img);
    let cov = coverage(&img, bg, EPS);
    let spread = quadrant_spread(&img, bg, EPS);
    let shells = radial_shell_occupancy(&img, bg, EPS);
    let flat = tonal_flatness(&img, bg, EPS);
    let boundary = boundary_density(&img, bg, EPS);
    let b_floor = boundary_floor(SystemKind::ParametricCurve);
    println!(
        "[blown out] against its own ground {bg:?}: coverage={cov:.4} (floor {floor:.2}) \
         quadrants={spread} shells={shells}/{RADIAL_SHELLS} flatness={flat:.4} \
         boundary={boundary:.4} (floor {b_floor:.2})"
    );
    assert!(
        flat > MAX_TONAL_FLATNESS,
        "the tonal question must still convict the blot once the reference is derived \
         from the frame, got {flat:.4}"
    );
    // (3) The true positive survives the gate becoming a conjunction (Plan 0119
    // Phase 3). Weakening a gate is only safe if the thing it was built to catch
    // is still caught, and this fixture is the whole of that evidence — it is
    // the sole anchor on the defect side of MAX_TONAL_FLATNESS, of ADR-0128's
    // conjunction, and of boundary_floor's default arm. Re-blessing it moves
    // three thresholds.
    assert!(
        boundary < b_floor,
        "the structural term must also convict the blot, or the conjunction has stopped \
         catching the defect it was weakened around: boundary {boundary:.4} against the \
         {b_floor:.2} floor"
    );
    assert!(
        bg.iter().take(3).any(|&c| c > EPS),
        "the fixture must be dense enough that its own tone is the modal band, or the two \
         lenses agree and (2) tests nothing: ground {bg:?}"
    );
}

/// The shipped preset this file uses as the witness that **term one is
/// load-bearing** — a frame under its family's boundary floor that term one
/// clears, so a structure-only gate would convict legitimate content.
///
/// `Sumi` is an ink wash: a fragment field with almost no perimeter per lit
/// pixel (`0.1008` at 2026-08-26, against the `0.31` default floor) and no tonal
/// problem at all. ADR-0130's landmine Negative names it specifically, as the
/// most natural mono-conversion target in the library and one of the 22 presets
/// held out of conviction by the tonal term alone.
///
/// **It is a witness, not a fixture.** If it is ever retuned above its floor or
/// retired, the property below is still true of the library and the fix is to
/// name another preset from the list `every_preset_draws_a_real_shape` prints on
/// every run — not to relax the assertion.
const STRUCTURELESS_BUT_TONED: &str = "Sumi";

/// **Neither term of the flatness conjunction is redundant**, asserted term by
/// term rather than inferred from a green suite (Plan 0119 Phase 3).
///
/// This test exists because of a specific way the change it guards can rot. After
/// Plan 0119 only a frame failing **both** terms is convicted, and the library
/// contains no such frame — so a regression in the conjunction's *wiring* (a
/// term dropped, an `&&` that became an `||`, a floor that stopped being
/// consulted) looks exactly like a healthy library. Every other test here would
/// stay green through it.
///
/// So the three frames below are read directly, and each term is shown to change
/// a verdict on its own:
///
/// 1. **Term two is load-bearing.** The held composition is *over* the tonal
///    ceiling — term one alone convicts it, which is the whole reason
///    `fragment_tiledmono` sat in `presets/pending/` for two plans — and clears
///    its boundary floor, so the conjunction acquits it.
/// 2. **Term one is load-bearing.** [`STRUCTURELESS_BUT_TONED`] is *under* its
///    boundary floor — term two alone convicts it — and clears the tonal
///    ceiling, so the conjunction acquits it.
/// 3. **The conjunction still convicts.** The frozen blot fails both terms.
///
/// Together those say the gate is a conjunction of two live terms, and that it
/// is neither vacuous nor a rename of the check it replaced.
#[test]
fn each_term_of_the_flatness_conjunction_is_load_bearing() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let blot = without_backdrop(blown_out());
    let held =
        without_backdrop(Preset::from_toml_str(HELD_OUT_TOML).expect("the frozen anchor parses"));
    let (blot_name, blot_system) = (blot.name.clone(), blot.system);
    let (held_name, held_system) = (held.name.clone(), held.system);

    let witness = default_presets()
        .into_iter()
        .find(|p| p.name == STRUCTURELESS_BUT_TONED)
        .map(without_backdrop)
        .unwrap_or_else(|| {
            panic!(
                "{STRUCTURELESS_BUT_TONED} is no longer in the shipped set — pick another \
                 preset from the under-their-floor list every_preset_draws_a_real_shape \
                 prints, and say in the commit which one and what it reads"
            )
        });
    let (witness_name, witness_system) = (witness.name.clone(), witness.system);

    renderer.set_presets(vec![blot, held, witness]);

    let frame = loud();
    let mut read = |name: &str, system: SystemKind| {
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture preset");
        let bg = ground(&img);
        let flat = tonal_flatness(&img, bg, EPS);
        let boundary = boundary_density(&img, bg, EPS);
        let b_floor = boundary_floor(system);
        println!(
            "[conjunction] {name:<22} flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2}) \
             boundary={boundary:.4} (floor {b_floor:.2})"
        );
        (flat, boundary, b_floor)
    };

    let (blot_flat, blot_boundary, blot_floor) = read(&blot_name, blot_system);
    let (held_flat, held_boundary, held_floor) = read(&held_name, held_system);
    let (wit_flat, wit_boundary, wit_floor) = read(&witness_name, witness_system);

    // (1) Term two is load-bearing: without it the held composition is convicted.
    assert!(
        held_flat > MAX_TONAL_FLATNESS,
        "the held composition must still be over the tonal ceiling, or term one alone \
         would already acquit it and term two is guarding nothing: flatness {held_flat:.4}"
    );
    assert!(
        held_boundary >= held_floor,
        "the held composition must clear its boundary floor, or the conjunction convicts \
         the preset it was built to admit: boundary {held_boundary:.4} against \
         {held_floor:.2}"
    );

    // (2) Term one is load-bearing: without it the witness is convicted.
    assert!(
        wit_boundary < wit_floor,
        "{witness_name} must still read under its boundary floor, or this frame no longer \
         witnesses that a structure-only gate would convict legitimate content — pick \
         another from the list the main gate prints: boundary {wit_boundary:.4} against \
         {wit_floor:.2}"
    );
    assert!(
        wit_flat <= MAX_TONAL_FLATNESS,
        "{witness_name} must clear the tonal ceiling, or it is not a witness for term one \
         at all: flatness {wit_flat:.4}"
    );

    // (3) And the conjunction still convicts the defect it was weakened around.
    assert!(
        blot_flat > MAX_TONAL_FLATNESS && blot_boundary < blot_floor,
        "the frozen blot must fail both terms: flatness {blot_flat:.4} against \
         {MAX_TONAL_FLATNESS:.2}, boundary {blot_boundary:.4} against {blot_floor:.2}"
    );
}

/// **`spectrum_ridge` exactly as it shipped broken**, recovered from
/// `git show 81190ac^:presets/spectrum_ridge.toml` — every table and every
/// binding byte-for-byte, comments stripped and the `name` suffixed so the
/// output reads clearly. Nothing here is tunable: this is the defect, frozen.
///
/// `scale = 3.20` is the whole of it. Tuned before ADR-0049 normalized the bands
/// to `0..1`, it afterwards multiplied a value roughly five times larger,
/// putting a driven element about **3.3 world units** up against a visible
/// half-height of `1.0`. Under [`loud`] the contour is off frame entirely and
/// the composite comes back empty except for `bg_vignette`.
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
    let scene_bg = ground(&scene);
    let cov = coverage(&scene, scene_bg, EPS);
    let spread = quadrant_spread(&scene, scene_bg, EPS);
    let shells = radial_shell_occupancy(&scene, scene_bg, EPS);
    println!(
        "[pre-repair ridge] new gate: ground={scene_bg:?} coverage={cov:.4} (floor {floor:.2}) \
         quadrants={spread} shells={shells}/{RADIAL_SHELLS}"
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
    let mid_cov = coverage(&quiet, ground(&quiet), EPS);
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

// ---------------------------------------------------------------------------
// The emptying canvas (Plan 0116 Phase 6, ADR-0126; the real family, Plan 0113
// Phase 6b)
// ---------------------------------------------------------------------------

/// **A canvas the music empties**, frozen the way [`blown_out`] and
/// [`pre_repair_spectrum_ridge`] are frozen — an inline fixture, not a shipped
/// preset — because what it stands for is a *defect*, and no shipped preset may
/// hold one.
///
/// Plan 0113 builds `shape_collage`, a scene whose element `density` falls with
/// the level, so a quiet passage leaves bare paper. **An emptied canvas and a
/// broken one are the same picture**, and against [`BLACK`] both read `coverage
/// = 1.0000`: the paper is not black, so every pixel counts as lit and the frame
/// scores as completely full. That is the false negative ADR-0126 was raised on,
/// and it is designed-in rather than hypothetical.
///
/// **Plan 0116 Phase 6 wrote this fixture on the attractor's `ink_*` remap**,
/// which reaches a paper-white frame the same way and was reachable while
/// `shape_collage` sat on an unmerged branch. Plan 0113 Phase 6b re-points it at
/// the family it was always about: everything below is the scene's own default
/// canvas — the authored suprematist arrangement, still, at `count` — and the
/// one binding is the lever the defect lives on.
///
/// Deliberately free of `time` terms: a fixture that is a pure function of its
/// excitation is one the reader can reason about, and the determinism rule in
/// `CLAUDE.md` applies to test content as much as to analysis.
fn emptying_canvas() -> Preset {
    Preset::from_toml_str(
        r##"
system = "shape_collage"
name   = "Emptying Canvas"

# The shipped palette's plateau shape, so the fixture is a flat graphic canvas
# and not a gradient with edges: each element's coordinate lands in the interior
# of one band, and the last band is the paper `paper` selects below.
[palette]
stops = [
  { at = 0.0000, color = "#111111" },
  { at = 0.1249, color = "#111111" },
  { at = 0.1251, color = "#8a1420" },
  { at = 0.2499, color = "#8a1420" },
  { at = 0.2501, color = "#96751e" },
  { at = 0.3749, color = "#96751e" },
  { at = 0.3751, color = "#1e3a8a" },
  { at = 0.4999, color = "#1e3a8a" },
  { at = 0.5001, color = "#1d5c34" },
  { at = 0.6249, color = "#1d5c34" },
  { at = 0.6251, color = "#4a4a4a" },
  { at = 0.7499, color = "#4a4a4a" },
  { at = 0.7501, color = "#5a1f4a" },
  { at = 0.8749, color = "#5a1f4a" },
  { at = 0.8751, color = "#d9d5c8" },
  { at = 1.0000, color = "#d9d5c8" },
]

[params]
paper = "0.9375"

# The whole fixture. `density` gates what fraction of the canvas is live, and
# the gate rounds *up* — so any positive value keeps at least one element and
# only an exact zero empties the canvas. At full drive this is 1.0 and the
# authored arrangement is entirely on the page; at a realistic level the clamp
# floors it and what is left is the paper it was drawn on.
density = "clamp(bass - 0.5, 0, 0.5) * 2"
"##,
    )
    .expect("the emptying-canvas fixture parses")
}

/// **Plan 0116 Phase 6, on the real family (Plan 0113 Phase 6b).** A canvas
/// with nothing left on it is convicted, where the purely areal predicate calls
/// the same frame completely full.
///
/// # Why both excitations are here
///
/// The gate reads `tonal_flatness` only at [`LOUD`], and at `LOUD` this canvas
/// is at its fullest — the statistic looks exactly where the defect cannot be.
/// The quiet capture buys one gate, [`MODERATE_MIN_COVERAGE`], and against
/// [`BLACK`] that gate reads `1.0000` on a bare page and passes it. So the
/// conviction has to come from the areal statistic at the *quiet* excitation,
/// which is the one place an emptied canvas actually occurs.
///
/// # The separation is a property, not a threshold
///
/// A bare ground has **no lit pixels at all** — not few, none — because every
/// pixel *is* the ground. A composition has some. Both frames below come from
/// the same preset at two levels, so the comparison is not between two authors'
/// taste, and **no number is invented for how sparse a legitimate composition
/// may be**. That is a content judgement and this file does not make it.
#[test]
fn a_canvas_the_music_empties_is_convicted_and_black_calls_it_full() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let name = "Emptying Canvas";
    renderer.set_presets(vec![without_backdrop(emptying_canvas())]);
    let floor = coverage_floor(SystemKind::ShapeCollage);

    let composed = renderer
        .capture_preset(name, &loud(), FRAMES)
        .expect("capture the emptying canvas at full drive");
    let bare = renderer
        .capture_preset(name, &excited(MODERATE), FRAMES)
        .expect("capture the emptying canvas at a realistic level");

    let (composed_bg, bare_bg) = (ground(&composed), ground(&bare));
    let composed_cov = coverage(&composed, composed_bg, EPS);
    let bare_cov = coverage(&bare, bare_bg, EPS);
    println!(
        "[emptying canvas] at {LOUD}: ground={composed_bg:?} coverage={composed_cov:.4} \
         (floor {floor:.2}) quadrants={} shells={}/{RADIAL_SHELLS} flatness={:.4}",
        quadrant_spread(&composed, composed_bg, EPS),
        radial_shell_occupancy(&composed, composed_bg, EPS),
        tonal_flatness(&composed, composed_bg, EPS),
    );
    println!(
        "[emptying canvas] at {MODERATE}: ground={bare_bg:?} coverage={bare_cov:.4} \
         (min {MODERATE_MIN_COVERAGE:.2}) quadrants={} shells={}/{RADIAL_SHELLS}",
        quadrant_spread(&bare, bare_bg, EPS),
        radial_shell_occupancy(&bare, bare_bg, EPS),
    );

    // (1) The driven frame is a real composition, or the fixture is just a
    // broken preset and convicting it demonstrates nothing about emptying.
    assert!(
        composed_cov >= floor && quadrant_spread(&composed, composed_bg, EPS) >= MIN_QUADRANTS,
        "the driven frame must pass the gate, or this fixture is a broken preset rather than \
         a canvas the music empties: coverage {composed_cov:.4} (floor {floor:.2}), {} quadrant(s)",
        quadrant_spread(&composed, composed_bg, EPS),
    );

    // (2) The emptied frame is convicted, at the excitation where emptying
    // actually happens.
    assert!(
        bare_cov < MODERATE_MIN_COVERAGE,
        "a canvas with nothing left on it must fail the quiet-excitation sentinel: \
         coverage {bare_cov:.4} >= {MODERATE_MIN_COVERAGE:.2}"
    );

    // (3) **The demonstration that this needed Phase 3.** Reverted onto the old
    // predicate the same frame reads completely full and clears the same gate,
    // so this test would pass while measuring nothing. `assert_eq` rather than a
    // bound: the paper is uniformly not-black, so the old lens does not merely
    // overestimate it, it saturates.
    let bare_under_black = coverage(&bare, BLACK, EPS);
    assert_eq!(
        bare_under_black, 1.0,
        "the whole premise is that a painted ground reads as a full frame against black; \
         if this is no longer 1.0 the fixture has stopped standing for the defect"
    );
    assert!(
        bare_under_black >= MODERATE_MIN_COVERAGE,
        "against black the emptied canvas must PASS the gate that convicts it above, or \
         Phase 3 repaired nothing: coverage {bare_under_black:.4}"
    );

    // (4) The property, stated without a threshold: the bare frame departs from
    // its own ground nowhere, the composed one somewhere. Everything between the
    // two is content and this file does not adjudicate it.
    assert_eq!(
        bare_cov, 0.0,
        "a bare ground is not sparsely covered, it is uncovered: every pixel is the ground \
         it would be measured against, got {bare_cov:.4}"
    );
    assert!(
        composed_cov > 0.0,
        "the composed frame must depart from its ground somewhere, got {composed_cov:.4}"
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
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
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
        let bg = ground(&img);
        let cov = coverage(&img, bg, EPS);
        let spread = quadrant_spread(&img, bg, EPS);
        let flat = tonal_flatness(&img, bg, EPS);
        let shells = radial_shell_occupancy(&img, bg, EPS);
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
/// **Non-vacuous, and by the case that motivated the gate**: the
/// pre-repair `spectrum_ridge` scores `0.0000` here as well as at `LOUD`,
/// asserted in
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

/// **"More audio must not mean less picture", measured.**
///
/// Captures every preset at [`MODERATE`] and [`LOUD`] and reports
/// `coverage(loud) / coverage(moderate)` — a ratio against the preset's own
/// quieter frame, because the scenes differ by an order of magnitude in how much
/// they paint and an absolute floor could not compare them.
///
/// # This ships as a report, not as a gate, and the measurement is why
///
/// The whole library, plus the pre-repair ridge as a control:
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
/// **No threshold on this axis convicts anything it was built for**, and that is
/// the trap for whoever tries to arm one. `Spectrum Comb` was the live candidate
/// and scores `1.0891` — it draws *more* when loud, because a comb roots every
/// bar on a shared baseline, so clipping the tips off the tallest bars costs a
/// rounding error of coverage while the body of the figure stays put. The layout
/// the check was designed around is the layout it cannot see. `Spectrum Corona`
/// is the same at `1.0514`. The pre-repair ridge is `0/0`: its contour is already
/// off frame at [`MODERATE`], and a ratio needs a denominator.
///
/// Meanwhile the only content near a plausible threshold is **correct** — `De
/// Jong` at `0.8552` and `Leviathan` at `0.9568` are the attractor family's
/// deliberate *peak buys structure* idiom, which ADR-0062 records as real. A gate
/// at `0.80` would sit `0.055` from `De Jong`, tight enough that a retune trips
/// it, while catching none of the three cases above.
///
/// So the ratio is printed, watched, and not enforced. What *is* enforced here is
/// [`MODERATE_MIN_COVERAGE`], the one property the second capture supports: a
/// preset must be a picture at a realistic level, not only when fully driven.
///
/// **The over-scale defect is real and this instrument does not reach it** —
/// pixel coverage is the wrong measure for a figure whose *tips* leave the frame,
/// because tips are almost no pixels. ADR-0067 names the successor, an in-frame
/// **geometry fraction**, kept there as the supplement for the line and spectrum
/// families; this measurement is the evidence that it is wanted.
///
/// **The ratio is reported per preset rather than as one sorted table**
/// (ADR-0157): with a test per preset there is no sweep to sort at the end of.
/// The reading itself is unchanged and every preset still prints one, so the
/// table above is reconstructed by sorting a run's lines rather than by the
/// test doing it.
fn louder_frame_is_reported_against_a_quieter_one(name: &str) {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let (presets, _meta) = sanity_roster();
    renderer.set_presets(presets);
    let (mid_frame, loud_frame) = (excited(MODERATE), excited(LOUD));

    let mid = renderer
        .capture_preset(name, &mid_frame, FRAMES)
        .expect("capture at moderate excitation");
    let mid_cov = coverage(&mid, ground(&mid), EPS);
    let loud = renderer
        .capture_preset(name, &loud_frame, FRAMES)
        .expect("capture at loud excitation");
    let loud_cov = coverage(&loud, ground(&loud), EPS);

    println!(
        "excitation ratio — coverage at {LOUD} over coverage at {MODERATE} (a report, not a \
         gate; this helper's doc comment says why):"
    );
    println!(
        "  {:>8.4}   {mid_cov:.4}   {loud_cov:.4}  {name}",
        ratio_of(loud_cov, mid_cov)
    );

    assert!(
        mid_cov >= MODERATE_MIN_COVERAGE,
        "{name} is not a picture at a realistic level: coverage {mid_cov:.4} at excitation \
         {MODERATE} is under {MODERATE_MIN_COVERAGE:.2} (it draws {loud_cov:.4} at {LOUD}). \
         Either a threshold in this preset sits above the level music occupies, or a \
         world-space param has already carried the figure out of frame by {MODERATE}"
    );
}

// One `#[test]` per shipped preset, generated by `core/build.rs` (ADR-0157).
include!(concat!(env!("OUT_DIR"), "/sanity_loudness_tests.rs"));

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

// ---------------------------------------------------------------------------
// Plan 0116 Phase 1 — what each candidate ground would say
// ---------------------------------------------------------------------------
//
// A measurement harness and nothing else. Every statistic above still measures
// against [`BLACK`]; this section adds no production behaviour and changes no
// verdict. It exists to put a table in front of Plan 0116 Phase 2, which is a
// **stop gate** — Plan 0116 may legitimately end there, and ADR-0126
// deliberately declines to name an estimator because the obvious one is
// already falsified.

/// How far in from each edge [`ground_modal_border`] samples, as a divisor of
/// the frame's shorter side. At this file's 96×96 capture that is a 6-pixel
/// margin holding 2160 of 9216 pixels — a population rather than a line, and
/// narrow enough that a centred figure cannot reach it.
const BORDER_DIVISOR: usize = 16;

/// Levels per channel [`ground_modal_rgb`] quantizes to before counting cells.
/// Sixteen makes a cell 16 wide on each axis, the same width [`TONE_BANDS`]
/// gives a luminance band — so the three live candidates differ by **what** they
/// cluster, not by how finely they cluster it.
const RGB_LEVELS: usize = 16;

/// **`fragment_tiledmono` frozen at the revision its calibration was measured
/// at**, recovered from `git show 26b20b3^:presets/pending/fragment_tiledmono.toml`
/// — every table and every binding byte-for-byte, comments stripped and the
/// `name` suffixed so the output reads clearly and so it cannot collide with the
/// shipped copy in a renderer that holds both. Nothing here is tunable.
///
/// # Why it is frozen rather than read from the file
///
/// An `include_str!` of the preset file is correct only while the preset is
/// held: under `presets/pending/` it is outside the embedded set
/// (`core/build.rs` globs `presets/*.toml` non-recursively per ADR-0022, so a
/// subdirectory is skipped by construction) and unreachable from
/// [`sanity_roster`], while still belonging in the table as the false positive
/// ADR-0126 was raised on and ADR-0128 was written about. Plan 0119 Phase 4
/// ships it into `presets/`, and repointing an `include_str!` at the new path
/// would be **worse than letting the move break the build**. This frame is the
/// composition-side anchor of [`boundary_floor`]'s default arm: `0.31` is the
/// midpoint of `0.2631` ([`blown_out`]) and this preset's `0.3602`. As ordinary
/// editable content, a routine preset tweak could move a gate constant with
/// nothing able to notice, because the constant would still read green.
/// ADR-0130's Decision requires **both** anchors frozen, and this is the second
/// one; `retired_mandalas` is the precedent for how.
///
/// The shipped copy goes on being judged by the gate like any other preset, and
/// the two are meant to agree. If they ever stop agreeing, the shipped preset
/// was retuned and [`boundary_floor`]'s default arm is re-opened with it — which
/// is exactly the event this freeze exists to make visible.
const HELD_OUT_TOML: &str = r##"
system = "fragment_field"
name = "Tiled Rosette Mono (frozen)"

[palette]
stops = [
  { at = 0.00, color = "#000000" },
  { at = 0.19, color = "#000000" },
  { at = 0.21, color = "#ffffff" },
  { at = 0.39, color = "#ffffff" },
  { at = 0.41, color = "#000000" },
  { at = 0.59, color = "#000000" },
  { at = 0.61, color = "#ffffff" },
  { at = 0.84, color = "#ffffff" },
  { at = 0.86, color = "#b00808" },
  { at = 1.00, color = "#b00808" },
]

[params]
kaleido_tile   = "2"
kaleido_radial = "1"
kaleido_inner  = "0.06"
kaleido_angle  = "time * 0.05"
kaleido_order  = "select(bass + mid + treb > 1.70, 9, 6)"

palette_steps   = "20"
palette_contour = "0"

warp  = "0.60 + sin(time * 0.037) * 0.12 + clamp(bass * 0.318, 0, 0.27)"
zoom  = "1.20 + clamp(treb * 0.30, 0, 0.18)"
pan_x = "sin(time * 0.013) * 0.06"
pan_y = "cos(time * 0.017) * 0.05"

glow  = "2.6"
flash = "0"

hue          = "0.08 + time * 0.011"
color_span   = "1.35"
color_center = "sin(time * 0.029) * 0.10"
saturation   = "1.0"

bloom_amount = "0"
trails       = "0"

[smoothing]
kaleido_order = 3.5

warp = 0.60
zoom = 0.50
"##;

/// One candidate ground estimator. **The roster is the deliverable, not a
/// choice** — Phase 2 chooses, from what these print.
struct GroundCandidate {
    /// Column name in the printed table.
    name: &'static str,
    /// One line on what it clusters, printed once above the table.
    note: &'static str,
    /// The reference tone this candidate would hand `is_lit`.
    pick: fn(&CaptureImage) -> [u8; 4],
}

/// The four columns ADR-0126 asks Phase 1 to table, control first so every
/// other column reads as a difference from it rather than as an absolute.
const GROUND_CANDIDATES: &[GroundCandidate] = &[
    GroundCandidate {
        name: "black",
        note: "the control - today's hardcoded reference (ADR-0067)",
        pick: ground_black,
    },
    GroundCandidate {
        name: "modal_luma",
        note: "mean RGB of the frame's most populous luminance band",
        pick: ground_modal_luma,
    },
    GroundCandidate {
        name: "modal_border",
        note: "the same, over border pixels only (see BORDER_DIVISOR)",
        pick: ground_modal_border,
    },
    GroundCandidate {
        name: "modal_rgb",
        note: "mean RGB of the most populous coarse RGB cell (see RGB_LEVELS)",
        pick: ground_modal_rgb,
    },
];

/// The control: what the lens uses today, whatever the frame contains.
fn ground_black(_img: &CaptureImage) -> [u8; 4] {
    BLACK
}

/// The estimator ADR-0126 names and falsifies — the frame's modal luminance
/// band. Tabled because "already falsified" is a claim this harness must be
/// able to check rather than inherit.
///
/// **It delegates to the production estimator**, so this column tables what
/// the gate actually does rather than a reproduction of it that can drift from
/// it. The only behaviour that adds is [`NO_GROUND`] on a frame with no
/// dominant band, and no shipped preset reaches it (see [`MIN_GROUND_SHARE`]),
/// so the table is unchanged from the one Phase 2 read.
///
/// [`NO_GROUND`]: lmv_core::render::metrics::NO_GROUND
/// [`MIN_GROUND_SHARE`]: lmv_core::render::metrics::MIN_GROUND_SHARE
fn ground_modal_luma(img: &CaptureImage) -> [u8; 4] {
    modal_ground(img)
}

/// The modal luminance band among **border** pixels only, on the argument that
/// a composition's ground reaches the frame edge and its figure usually does
/// not.
fn ground_modal_border(img: &CaptureImage) -> [u8; 4] {
    modal_border_band(img)
}

/// The modal **RGB** cell rather than the modal luminance band. Luminance
/// collapses hue, so a two-colour world at equal brightness has one modal band
/// and two grounds; this separates them at the cost of a sparser histogram.
fn ground_modal_rgb(img: &CaptureImage) -> [u8; 4] {
    let cells = RGB_LEVELS * RGB_LEVELS * RGB_LEVELS;
    let mut counts = vec![0u64; cells];
    let mut sums = vec![[0u64; 3]; cells];
    for px in img.rgba.chunks_exact(4) {
        let q = |c: usize| (px[c] as usize * RGB_LEVELS / 256).min(RGB_LEVELS - 1);
        let cell = (q(0) * RGB_LEVELS + q(1)) * RGB_LEVELS + q(2);
        counts[cell] += 1;
        for c in 0..3 {
            sums[cell][c] += px[c] as u64;
        }
    }
    modal_mean(&counts, &sums)
}

/// Mean RGB of the most populous luminance band among the frame's border
/// pixels — the whole-frame form is `metrics::modal_ground`.
///
/// The **mean of the band's members**, not the band's centre: an ink-on-paper
/// world's paper is a specific off-white, and rounding it to the middle of a
/// 16-level band would hand `is_lit` a reference the frame does not contain.
fn modal_border_band(img: &CaptureImage) -> [u8; 4] {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 {
        return BLACK;
    }
    let margin = (w.min(h) / BORDER_DIVISOR).max(1);
    let mut counts = [0u64; TONE_BANDS];
    let mut sums = [[0u64; 3]; TONE_BANDS];
    for (i, px) in img.rgba.chunks_exact(4).enumerate() {
        let (x, y) = (i % w, i / w);
        if x >= margin && y >= margin && x + margin < w && y + margin < h {
            continue;
        }
        let band = (((sanity_luma(px) / 256.0) * TONE_BANDS as f32) as usize).min(TONE_BANDS - 1);
        counts[band] += 1;
        for c in 0..3 {
            sums[band][c] += px[c] as u64;
        }
    }
    modal_mean(&counts, &sums)
}

/// Mean RGB of the most populous cell in a parallel `counts` / `sums` pair, or
/// [`BLACK`] when nothing was counted.
///
/// **On a tie `max_by_key` keeps the last maximum**, so a frame with no dominant
/// tone gets a deterministic but arbitrary answer — which is the failure mode
/// ADR-0126's Consequences names and Phase 3's done-when has to define rather
/// than discover. It is left arbitrary here on purpose: defining it is a
/// production decision, and this phase makes none.
fn modal_mean(counts: &[u64], sums: &[[u64; 3]]) -> [u8; 4] {
    let Some((best, &n)) = counts.iter().enumerate().max_by_key(|&(_, &n)| n) else {
        return BLACK;
    };
    if n == 0 {
        return BLACK;
    }
    let s = sums[best];
    [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8, 255]
}

/// Rec.601 luma — the same weights `metrics::tonal_flatness` buckets by. That
/// helper is private to `core`, so it is restated here rather than widening a
/// production surface for a harness Phase 2 may discard.
fn sanity_luma(px: &[u8]) -> f32 {
    0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32
}

/// What one candidate ground said about one preset, kept only as far as the
/// cross-column diff needs it.
///
/// The four statistics themselves are **printed** on the preset's own row and
/// not retained: a reader compares them by eye down the four candidate lines,
/// and the only thing the summary computes across columns is whether the
/// reference moved and whether the verdict did.
#[derive(Clone)]
struct GroundRow {
    /// The reference tone this candidate handed `is_lit`.
    reference: [u8; 4],
    /// Empty means this candidate would pass the preset.
    failures: Vec<String>,
}

/// `every_preset_draws_a_real_shape`'s verdict, restated over a supplied
/// reference rather than [`BLACK`] — the structural rescue included, since a
/// candidate that moves `coverage` moves what the rescue is asked about.
///
/// [`KNOWN_FLAT`] is not consulted: it is empty, and an exemption roster would
/// hide exactly the verdict changes this table exists to count.
fn ground_verdict_loud(
    system: SystemKind,
    cov: f32,
    spread: u8,
    shells: usize,
    flat: f32,
) -> Vec<String> {
    let mut failures = Vec::new();
    let floor = coverage_floor(system);
    if cov < floor && shells < MIN_STRUCTURAL_SHELLS {
        failures.push(format!(
            "blank (cov {cov:.4} < {floor:.2}, shells {shells} < {MIN_STRUCTURAL_SHELLS})"
        ));
    }
    if spread < MIN_QUADRANTS {
        failures.push(format!("dot ({spread} quadrant(s) < {MIN_QUADRANTS})"));
    }
    if flat > MAX_TONAL_FLATNESS {
        failures.push(format!("flat ({flat:.4} > {MAX_TONAL_FLATNESS:.2})"));
    }
    failures
}

/// The one gate the quieter capture buys today ([`MODERATE_MIN_COVERAGE`]),
/// restated over a supplied reference.
fn ground_verdict_moderate(cov: f32) -> Vec<String> {
    if cov < MODERATE_MIN_COVERAGE {
        vec![format!(
            "not a picture at {MODERATE} (cov {cov:.4} < {MODERATE_MIN_COVERAGE:.2})"
        )]
    } else {
        Vec::new()
    }
}

/// Name every preset whose verdict moves under each candidate, against the
/// control column — **the number Phase 2 decides on**. ADR-0126's own
/// falsification of naive modal tone is a count of exactly this shape (17 of
/// 41), so a candidate is judged by how much of the library it re-bases and by
/// how many verdicts that costs, not by how good its idea sounds.
fn report_ground_verdict_changes(
    label: &str,
    meta: &[(String, SystemKind)],
    rows: &[Vec<GroundRow>],
) {
    let Some(control) = rows.first() else {
        return;
    };
    println!();
    println!("verdict changes at {label}, against the `black` control:");
    for (ci, cand) in GROUND_CANDIDATES.iter().enumerate().skip(1) {
        let Some(column) = rows.get(ci) else {
            continue;
        };
        let mut to_fail = Vec::new();
        let mut to_pass = Vec::new();
        let mut rebased = 0usize;
        for (i, row) in column.iter().enumerate() {
            let Some(base) = control.get(i) else {
                continue;
            };
            let name = meta.get(i).map(|(n, _)| n.as_str()).unwrap_or("?");
            // "Re-based" is `is_lit(reference, BLACK, EPS)`: this candidate
            // picked a reference the old lens would have called lit, so every
            // statistic downstream is answering a different question.
            if row.reference.iter().take(3).any(|&c| c > EPS) {
                rebased += 1;
            }
            match (base.failures.is_empty(), row.failures.is_empty()) {
                (true, false) => to_fail.push(format!("{name} -> {}", row.failures.join("; "))),
                (false, true) => {
                    to_pass.push(format!("{name} (was: {})", base.failures.join("; ")))
                }
                _ => {}
            }
        }
        println!(
            "  {:<13} re-based {rebased}/{} preset(s);  pass->fail {};  fail->pass {}",
            cand.name,
            column.len(),
            to_fail.len(),
            to_pass.len(),
        );
        for entry in &to_fail {
            println!("      pass->fail  {entry}");
        }
        for entry in &to_pass {
            println!("      fail->pass  {entry}");
        }
    }
}

/// **Plan 0116 Phase 1.** Print, for every preset in the embedded set plus the
/// held-out `Tiled Rosette Mono`, at both [`LOUD`] and [`MODERATE`], the
/// reference tone each candidate ground estimator picks and the four statistics
/// that follow from it — beside the [`BLACK`] control the lens uses today.
///
/// # This gates nothing, and cannot
///
/// It is `#[ignore]`d and contains no assertion. That is the phase's own
/// done-when: a harness built to inform a **stop gate** must not be able to
/// redden CI on its own, or the gate is decided by whichever candidate happens
/// to be green. It is also 82 WARP captures, which is a second reason not to
/// put it in the everyday loop.
///
/// Run it with:
///
/// ```text
/// cargo nextest run -p lmv-core --test sanity --run-ignored all \
///     each_candidate_ground_is_tabled_against_the_library --no-capture
/// ```
///
/// # What is missing from the table, said here rather than silently
///
/// **`shape_collage` contributes no row.** It is the family that motivates this
/// work — Plan 0113 Phase 6 builds a canvas the music empties, and an emptied
/// canvas is pixel-for-pixel a broken one — and it has not merged: it lives on
/// `plan-0113-shape-collage` in its own worktree. So the estimator is being
/// chosen from a library that contains no scene painting its own paper across
/// every pixel *except* the attractor's ink duotone and the twelve presets that
/// already read `coverage = 1.0000`. Those twelve are the nearest evidence
/// available, and they are what the table can speak to.
#[test]
#[ignore = "measurement, not a gate: Plan 0116 Phase 1 informs a human stop gate, and it is 82 WARP captures"]
fn each_candidate_ground_is_tabled_against_the_library() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    // The embedded roster measured exactly as the gate measures it (backdrops
    // suppressed, ADR-0067), plus the frozen `Tiled Rosette Mono` the estimator
    // has to get right — held out of the set when this table was taken, shipped
    // since (Plan 0119 Phase 4), and kept here as the frozen copy so the reading
    // does not move with the file. Anything else would table a different
    // measurement than the one Plan 0116 Phase 3 changed.
    let (mut presets, mut meta) = sanity_roster();
    let held =
        without_backdrop(Preset::from_toml_str(HELD_OUT_TOML).expect("the frozen anchor parses"));
    meta.push((held.name.clone(), held.system));
    presets.push(held);
    renderer.set_presets(presets);

    println!("{}", "=".repeat(78));
    println!("Plan 0116 Phase 1 - candidate ground estimators against the shipped library");
    println!("{}", "=".repeat(78));
    println!(
        "roster: {} preset(s) - the embedded set plus the held-out \
         presets/pending/fragment_tiledmono.toml",
        meta.len()
    );
    println!(
        "NOT IN THIS TABLE: shape_collage. Plan 0113 has not merged (branch \
         plan-0113-shape-collage), so"
    );
    println!("  the family this work exists for contributes no row - see this test's doc comment.");
    println!("candidates:");
    for cand in GROUND_CANDIDATES {
        println!("  {:<13} {}", cand.name, cand.note);
    }

    for (label, level) in [("LOUD", LOUD), ("MODERATE", MODERATE)] {
        let frame = excited(level);
        let mut rows: Vec<Vec<GroundRow>> = vec![Vec::new(); GROUND_CANDIDATES.len()];
        println!();
        println!("-- excitation {label} ({level}) {}", "-".repeat(44));
        for (name, system) in &meta {
            let img = renderer
                .capture_preset(name, &frame, FRAMES)
                .expect("capture preset");
            println!("[{}] {name}", system_name(*system));
            for (ci, cand) in GROUND_CANDIDATES.iter().enumerate() {
                let bg = (cand.pick)(&img);
                let cov = coverage(&img, bg, EPS);
                let spread = quadrant_spread(&img, bg, EPS);
                let shells = radial_shell_occupancy(&img, bg, EPS);
                let flat = tonal_flatness(&img, bg, EPS);
                let failures = if label == "LOUD" {
                    ground_verdict_loud(*system, cov, spread, shells, flat)
                } else {
                    ground_verdict_moderate(cov)
                };
                let verdict = if failures.is_empty() {
                    "PASS".to_string()
                } else {
                    format!("FAIL: {}", failures.join("; "))
                };
                println!(
                    "   {:<13} ref ({:>3},{:>3},{:>3})  cov {cov:.4}  quad {spread}  \
                     shells {shells:>2}/{RADIAL_SHELLS}  flat {flat:.4}  {verdict}",
                    cand.name, bg[0], bg[1], bg[2],
                );
                if let Some(column) = rows.get_mut(ci) {
                    column.push(GroundRow {
                        reference: bg,
                        failures,
                    });
                }
            }
        }
        report_ground_verdict_changes(label, &meta, &rows);
    }
}

// ---------------------------------------------------------------------------
// What separates a composition from a blot
// ---------------------------------------------------------------------------
//
// A measurement harness and nothing else, the same shape as the ground-estimator
// harness above and for the same reason: ADR-0126 named a mechanism without
// measuring it and it was falsified one plan later. ADR-0128 named a second —
// that a picture is a blot only if it is tonally flat *and* structureless — and
// this section exists to find out whether any statistic actually says that,
// before a live gate is weakened on the strength of it.
//
// The three pixel-scale candidates were measured and all three failed. ADR-0129
// read that failure as one axis chosen three times: they are computed on the
// binary lit mask at 96x96, where a particle cloud's mask *is* noise, and noise
// is jagged — so the blot outscores the library's own sparsest legitimate
// content on every one of them. Its candidate is the fourth column family here,
// `tile@N`, and it is deliberately orthogonal: tone rather than mask, all pixels
// rather than lit ones, composition scale rather than pixel scale.
//
// ADR-0129 also corrects how the columns are judged. A conjunction's second term
// is only ever asked about frames that already failed the first, so a frame
// "in the gap" disqualifies a candidate only if its own `tonal_flatness` is
// above the ceiling. Conditioned that way the calibration population has two
// members, and [`report_structure_separation`] prints it so that is visible
// rather than assumed.
//
// The candidates lived here rather than in `core/src/render/metrics.rs`, exactly
// as the ground estimators did, because all but one would be discarded and a
// discarded candidate that ships as a `pub` production statistic is worse than
// no measurement.
//
// **One of them stopped being a candidate.** Plan 0119 Phase 2 read this table
// and chose `boundary` — the control ADR-0129 named and expected to lose, which
// won once criterion 2 was conditioned correctly (ADR-0130). It now lives in
// `metrics::boundary_density` as gate behaviour, and its column here reads that
// function rather than a copy. `components`, `sobel` and the whole `tile@N`
// sweep are **discarded candidates** and stay in this file for exactly the
// reason the paragraph above gives.

/// Whether a pixel departs from `bg` on any RGB channel by more than [`EPS`] —
/// `metrics::is_lit`, which is private to `core`, restated for the same reason
/// [`sanity_luma`] is.
fn sanity_is_lit(px: &[u8], bg: [u8; 4]) -> bool {
    px.iter()
        .zip(bg.iter())
        .take(3)
        .any(|(&c, &b)| c.abs_diff(b) > EPS)
}

/// The lit mask of a frame against its own ground, as one bool per pixel.
fn lit_mask(img: &CaptureImage) -> Vec<bool> {
    let bg = ground(img);
    img.rgba
        .chunks_exact(4)
        .map(|px| sanity_is_lit(px, bg))
        .collect()
}

/// One candidate structural statistic. **The roster is the deliverable, not a
/// choice** — the stop condition decides, from what these print, and it can end
/// Plan 0116.
struct StructureCandidate {
    /// Column name in the printed table.
    name: &'static str,
    /// One line on what it counts, printed once above the table.
    note: &'static str,
    /// Higher must mean *more* structured, so every column is read in one
    /// direction.
    measure: Measure,
}

/// How a column is read off a frame.
///
/// The pixel-scale candidates are plain functions of the whole frame. The
/// ADR-0129 candidate carries a grid size instead, because **its tile count is
/// swept rather than chosen** — each grid in [`TILE_SWEEP`] is its own column, so
/// the gate reads a curve and can tell a plateau from a fitted number.
enum Measure {
    /// Read over the whole frame — the control and the three Plan 0116 Phase 8
    /// candidates.
    Whole(fn(&CaptureImage) -> f32),
    /// [`modal_band_tile_transitions`] at this many tiles per side.
    Tiled(usize),
}

impl StructureCandidate {
    /// This column's reading of one frame.
    fn read(&self, img: &CaptureImage) -> f32 {
        match self.measure {
            Measure::Whole(f) => f(img),
            Measure::Tiled(tiles) => modal_band_tile_transitions(img, tiles),
        }
    }
}

/// Tiles per side [`modal_band_tile_transitions`] is swept over, every one of
/// which divides the [`SIZE`] capture exactly.
///
/// Printed as five separate columns rather than reduced to one, because a
/// verdict that flips between adjacent grids means the statistic is
/// resolution-coupled — which is a *stop*, and only visible as a curve.
const TILE_SWEEP: [usize; 5] = [4, 6, 8, 12, 16];

/// The control and the three Plan 0116 Phase 8 candidates, control first so
/// every other column reads as a difference from it rather than as an absolute,
/// followed by the ADR-0129 candidate once per [`TILE_SWEEP`] grid.
const STRUCTURE_CANDIDATES: &[StructureCandidate] = &[
    StructureCandidate {
        name: "flatness^-1",
        note: "the control - 1 - tonal_flatness, so high means structured like the rest",
        measure: Measure::Whole(inverse_flatness),
    },
    StructureCandidate {
        name: "boundary",
        note: "share of lit pixels with an unlit 4-neighbour (perimeter over lit area) - SHIPPED as the second term, and this column now reads metrics::boundary_density",
        measure: Measure::Whole(production_boundary_density),
    },
    StructureCandidate {
        name: "components",
        note: "4-connected components in the lit mask, per thousand lit pixels",
        measure: Measure::Whole(component_density),
    },
    StructureCandidate {
        name: "sobel",
        note: "mean |Sobel| over the binary lit mask at capture resolution",
        measure: Measure::Whole(mask_sobel_density),
    },
    StructureCandidate {
        name: "tile@4",
        note: "ADR-0129: differing adjacent modal-band tiles on a 4x4 grid (24px tiles)",
        measure: Measure::Tiled(4),
    },
    StructureCandidate {
        name: "tile@6",
        note: "ADR-0129: differing adjacent modal-band tiles on a 6x6 grid (16px tiles)",
        measure: Measure::Tiled(6),
    },
    StructureCandidate {
        name: "tile@8",
        note: "ADR-0129: differing adjacent modal-band tiles on an 8x8 grid (12px tiles)",
        measure: Measure::Tiled(8),
    },
    StructureCandidate {
        name: "tile@12",
        note: "ADR-0129: differing adjacent modal-band tiles on a 12x12 grid (8px tiles)",
        measure: Measure::Tiled(12),
    },
    StructureCandidate {
        name: "tile@16",
        note: "ADR-0129: differing adjacent modal-band tiles on a 16x16 grid (6px tiles)",
        measure: Measure::Tiled(16),
    },
];

/// The control, inverted so it reads in the same direction as the other three:
/// `1 - tonal_flatness`, high for a picture with tonal structure.
fn inverse_flatness(img: &CaptureImage) -> f32 {
    1.0 - tonal_flatness(img, ground(img), EPS)
}

/// The `boundary` column, reading the **production** statistic.
///
/// It was a local helper here while it was a candidate; Plan 0119 Phase 3 moved
/// it into `metrics::boundary_density` because it is gate behaviour now. The
/// column stays, pointed at the shipped function, so the table Phase 2 decided
/// on is still re-runnable and cannot drift away from what the gate reads.
fn production_boundary_density(img: &CaptureImage) -> f32 {
    boundary_density(img, ground(img), EPS)
}

/// 4-connected components of the lit mask, per thousand lit pixels.
///
/// Normalized by lit area rather than reported raw, so a dense figure and a
/// sparse one are comparable: one solid mass reads near zero however large it
/// is, and a field of separate marks reads high however many there are.
fn component_density(img: &CaptureImage) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 {
        return 0.0;
    }
    let mask = lit_mask(img);
    let lit = mask.iter().filter(|&&b| b).count();
    if lit == 0 {
        return 0.0;
    }
    let mut seen = vec![false; mask.len()];
    let mut components = 0u64;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        components += 1;
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut visit = |nx: usize, ny: usize| {
                let j = ny * w + nx;
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(x - 1, y);
            }
            if x + 1 < w {
                visit(x + 1, y);
            }
            if y > 0 {
                visit(x, y - 1);
            }
            if y + 1 < h {
                visit(x, y + 1);
            }
        }
    }
    components as f32 * 1000.0 / lit as f32
}

/// Mean Sobel gradient magnitude over the **binary** lit mask, at capture
/// resolution.
///
/// The binary mask rather than the grayscale frame on purpose: a smooth luminous
/// field has gradients everywhere and would read as structured, and the question
/// ADR-0128 asks is about the *shape* of what is lit, not its shading. Border
/// pixels stay zero (no wrap), matching `metrics::sobel`.
fn mask_sobel_density(img: &CaptureImage) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mask = lit_mask(img);
    let at = |x: usize, y: usize| -> f32 { f32::from(u8::from(mask[y * w + x])) };
    let mut sum = 0.0f64;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1);
            let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1);
            sum += f64::from((gx * gx + gy * gy).sqrt());
        }
    }
    (sum / ((w - 2) * (h - 2)) as f64) as f32
}

/// Share of adjacent tile pairs whose **modal luminance band** differs, on a
/// `tiles x tiles` grid over the whole frame — ADR-0129's composition-scale
/// candidate.
///
/// Two departures from the three candidates above, and both are the point rather
/// than a variation:
///
/// - **Tone, not the binary mask.** Those three throw tone away and keep shape;
///   this keeps tone and throws away fine shape. The tiling is a low-pass
///   filter, and the signal that defeated all three is high-frequency: a
///   particle blot's raggedness lives *below* the tile and averages away, while
///   a tiled ornament or a stroke lattice has a period *above* it and survives.
/// - **All pixels, not lit ones.** In a duotone the ink and the paper are both
///   the composition. Restricting to lit pixels is what makes `tonal_flatness`
///   structurally unable to see a duotone, and inheriting that would inherit the
///   defect.
///
/// Banding is [`TONE_BANDS`] over `0..255` by [`sanity_luma`] — the same binning
/// `metrics::tonal_flatness` uses, so a change to `TONE_BANDS` moves both. A tile
/// whose two most populous bands tie takes the **lower** band index, so the
/// reading is deterministic rather than iteration-order dependent.
///
/// **Tile bounds are `i * w / tiles`, not `i * (w / tiles)`.** Every grid in
/// [`TILE_SWEEP`] divides the [`SIZE`] capture exactly, so today no tile is
/// ragged; if a future capture size does not divide, the remainder is spread one
/// pixel at a time across the grid rather than piled onto a last, wider tile.
/// No tile is ever empty while `tiles <= min(width, height)`.
///
/// Adjacency is 4-neighbour with no wrap, so the denominator is
/// `2 * tiles * (tiles - 1)`. `0.0` for a frame with no adjacent pairs at all.
fn modal_band_tile_transitions(img: &CaptureImage, tiles: usize) -> f32 {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || tiles < 2 || tiles > w.min(h) {
        return 0.0;
    }
    let bound = |i: usize, n: usize| i * n / tiles;
    let mut bands = vec![0usize; tiles * tiles];
    for ty in 0..tiles {
        for tx in 0..tiles {
            let mut counts = [0u64; TONE_BANDS];
            for y in bound(ty, h)..bound(ty + 1, h) {
                for x in bound(tx, w)..bound(tx + 1, w) {
                    let Some(px) = img.rgba.get((y * w + x) * 4..(y * w + x) * 4 + 4) else {
                        continue;
                    };
                    let band = ((sanity_luma(px) / 256.0) * TONE_BANDS as f32) as usize;
                    counts[band.min(TONE_BANDS - 1)] += 1;
                }
            }
            let (mut best, mut most) = (0usize, 0u64);
            for (band, &n) in counts.iter().enumerate() {
                if n > most {
                    most = n;
                    best = band;
                }
            }
            bands[ty * tiles + tx] = best;
        }
    }
    let mut differing = 0u64;
    for ty in 0..tiles {
        for tx in 0..tiles {
            let here = bands[ty * tiles + tx];
            if tx + 1 < tiles && bands[ty * tiles + tx + 1] != here {
                differing += 1;
            }
            if ty + 1 < tiles && bands[(ty + 1) * tiles + tx] != here {
                differing += 1;
            }
        }
    }
    let pairs = 2 * tiles * (tiles - 1);
    if pairs == 0 {
        return 0.0;
    }
    differing as f32 / pairs as f32
}

/// One frame in the table, with the role it plays in the stop condition.
struct StructureRow {
    name: String,
    /// One value per [`STRUCTURE_CANDIDATES`] column.
    values: Vec<f32>,
    /// The frame's own `tonal_flatness`, which is **not** a candidate column:
    /// it is the first term, and criterion 2 of ADR-0129's stop condition cannot
    /// be read without it — a frame in the gap only disqualifies a candidate if
    /// the conjunction could ever reach it.
    flatness: f32,
    /// `Some(true)` = must read structureless, `Some(false)` = must read
    /// structured, `None` = shipped content, which must land outside the gap
    /// between those two.
    is_blot: Option<bool>,
}

/// Print, for the frozen blot fixture, the held-out `Tiled Rosette Mono`, the
/// three frozen thin-stroke mandalas and the whole shipped library, what each
/// candidate structural statistic says — beside `tonal_flatness`, the statistic
/// ADR-0128 proposes to add a second term to.
///
/// # This gates nothing, and cannot
///
/// It is `#[ignore]`d and contains no assertion: a report built to inform a
/// **stop gate** must not be able to redden CI on its own, or the gate is
/// decided by whichever candidate happens to be green.
///
/// # The stop condition, which is mechanical
///
/// ADR-0129's three parts, all of which must hold for one candidate column —
/// separation, nothing convictable in the gap, and a threshold that convicts the
/// blot with margin. Two things the report does rather than assumes: the
/// gap criterion is judged over the **conditional** population (frames in the
/// gap below [`MAX_TONAL_FLATNESS`] are printed with their flatness, as reported
/// and not disqualifying), and the third part's number is a measurement between
/// the two frozen fixtures rather than a derived floor, so the margin and the
/// column's legitimate spread are printed beside it. If no candidate passes all
/// three the search stops, and the report prints that verdict per candidate
/// rather than leaving it to be read off the rows.
///
/// # Thin-stroke content is in the table on purpose
///
/// A hairline over a 46-fold ornament aliases to almost nothing at 96×96, which
/// is what made `coverage` a halo-meter (design-backlog 0072). A boundary-length
/// measure is exactly the kind of statistic that could inherit that failure, so
/// the three frozen [`retired_mandalas`] are rows here rather than assumed safe.
///
/// Run it with:
///
/// ```text
/// cargo nextest run -p lmv-core --test sanity --run-ignored all ///     each_structure_candidate_is_tabled_against_the_library --no-capture
/// ```
#[test]
#[ignore = "measurement, not a gate: this informs a mechanical stop condition"]
fn each_structure_candidate_is_tabled_against_the_library() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let (mut presets, meta) = sanity_roster();
    let mut roles: Vec<(String, Option<bool>)> =
        meta.iter().map(|(n, _)| (n.clone(), None)).collect();

    // The blot that must read structureless.
    let blot = without_backdrop(blown_out());
    roles.push((blot.name.clone(), Some(true)));
    presets.push(blot);

    // The composition that must read structured — the preset ADR-0128 exists
    // for, and the composition-side anchor of boundary_floor's default arm.
    // It ships now (Plan 0119 Phase 4), so `sanity_roster` carries a copy of it
    // too; this row is the FROZEN one, and the two appearing side by side in the
    // table is the point rather than a duplicate.
    let held =
        without_backdrop(Preset::from_toml_str(HELD_OUT_TOML).expect("the frozen anchor parses"));
    roles.push((held.name.clone(), Some(false)));
    presets.push(held);

    // Thin-stroke content, which a boundary measure must not mistake for a blot.
    for mandala in retired_mandalas() {
        let mandala = without_backdrop(mandala);
        roles.push((mandala.name.clone(), None));
        presets.push(mandala);
    }

    renderer.set_presets(presets);

    println!("{}", "=".repeat(78));
    println!("candidate structural statistics, at LOUD");
    println!("{}", "=".repeat(78));
    println!("candidates (higher = more structured, in every column):");
    for cand in STRUCTURE_CANDIDATES {
        println!("  {:<13} {}", cand.name, cand.note);
    }
    println!("  tile sweep: {TILE_SWEEP:?} tiles per side at {SIZE}x{SIZE}, all exact divisors");
    println!("`flat=` is the frame's own tonal_flatness, ceiling {MAX_TONAL_FLATNESS:.2}.");
    println!("  It is term one and not a candidate: it decides whether a row reaches term");
    println!("  two at all (ADR-0129 criterion 2).");
    println!(
        "roles: [blot] must read lowest, [comp] must read above it, the rest must not fall \
         between them"
    );
    println!("  while being flat enough for the conjunction to reach.");
    println!(
        "NOTE: `Sumi`, `Whorl`, `Supernova` and `Neon Tunnel` are the four groundless luminous"
    );
    println!("  fields ADR-0128 records as the same open question - read their rows deliberately.");
    println!();

    // Where the tile sweep starts, so a row prints the whole-frame candidates on
    // one line and the curve on the next instead of one 130-column line.
    let tiled_from = STRUCTURE_CANDIDATES
        .iter()
        .position(|c| matches!(c.measure, Measure::Tiled(_)))
        .unwrap_or(STRUCTURE_CANDIDATES.len());

    let frame = loud();
    let mut rows: Vec<StructureRow> = Vec::new();
    for (name, is_blot) in &roles {
        let img = renderer
            .capture_preset(name, &frame, FRAMES)
            .expect("capture preset");
        let values: Vec<f32> = STRUCTURE_CANDIDATES.iter().map(|c| c.read(&img)).collect();
        let flatness = tonal_flatness(&img, ground(&img), EPS);
        let role = match is_blot {
            Some(true) => "[blot]",
            Some(false) => "[comp]",
            None => "      ",
        };
        let printed: Vec<String> = STRUCTURE_CANDIDATES
            .iter()
            .zip(values.iter())
            .map(|(c, v)| format!("{}={v:.4}", c.name))
            .collect();
        println!(
            "{role} {name:<22} flat={flatness:.4}  {}",
            printed[..tiled_from].join("  ")
        );
        println!("{:30}{}", "", printed[tiled_from..].join("  "));
        rows.push(StructureRow {
            name: name.clone(),
            values,
            flatness,
            is_blot: *is_blot,
        });
    }

    report_structure_separation(&rows);
}

/// Per candidate, ADR-0129's three-part stop condition — **the whole criterion**,
/// printed rather than left to the reader.
///
/// The correction over the criterion this instrument shipped with is criterion 2:
/// a frame between the two anchors disqualifies a candidate only if the
/// conjunction could ever reach it, which means only if its own
/// `tonal_flatness` is above [`MAX_TONAL_FLATNESS`]. Everything else in the gap
/// is printed with its flatness and marked as reported, because a reading that
/// cannot be checked is a reading that gets believed.
fn report_structure_separation(rows: &[StructureRow]) {
    println!();
    println!("separation at LOUD (ADR-0129's corrected stop condition):");

    let find = |want: bool| rows.iter().find(|r| r.is_blot == Some(want));
    let (Some(blot), Some(comp)) = (find(true), find(false)) else {
        println!("  the table is missing one of its two anchors — nothing to decide on");
        return;
    };

    // Criterion 2's population, printed once: the frames a second term can
    // reach at all. ADR-0129 records this as two members, one of which is the
    // preset the change exists to admit — if it is ever larger, the calibration
    // stopped being a two-point one and the threshold's docstring is stale.
    let reachable: Vec<&StructureRow> = rows
        .iter()
        .filter(|r| r.flatness > MAX_TONAL_FLATNESS)
        .collect();
    println!("  the conditional population - every frame a second term can reach at all");
    println!("  (its own flatness is over the {MAX_TONAL_FLATNESS:.2} ceiling):");
    for r in &reachable {
        let role = match r.is_blot {
            Some(true) => "[blot]",
            Some(false) => "[comp]",
            None => "      ",
        };
        println!("      {role} {:<22} flat={:.4}", r.name, r.flatness);
    }
    println!(
        "      {} member(s). Every other frame passes term one and is never",
        reachable.len()
    );
    println!("      asked the second question.");
    println!();

    for (ci, cand) in STRUCTURE_CANDIDATES.iter().enumerate() {
        let (Some(&lo), Some(&hi)) = (blot.values.get(ci), comp.values.get(ci)) else {
            continue;
        };

        // (1) Separation.
        if lo >= hi {
            println!(
                "  {:<13} (1) NOT SEPARATED: {} reads {lo:.4}, {} reads {hi:.4}   -> FAILS",
                cand.name, blot.name, comp.name,
            );
            continue;
        }
        println!(
            "  {:<13} (1) separated by {:.4} ({lo:.4} -> {hi:.4}), a {:.2}x margin",
            cand.name,
            hi - lo,
            if lo > 0.0 { hi / lo } else { f32::INFINITY },
        );

        // (2) Nothing convictable in the gap.
        let in_gap: Vec<&StructureRow> = rows
            .iter()
            .filter(|r| r.is_blot.is_none())
            .filter(|r| r.values.get(ci).is_some_and(|&v| v > lo && v < hi))
            .collect();
        let convictable = in_gap
            .iter()
            .filter(|r| r.flatness > MAX_TONAL_FLATNESS)
            .count();
        println!(
            "      (2) {} frame(s) in the gap, {convictable} of them above the flatness ceiling",
            in_gap.len(),
        );
        for r in &in_gap {
            let verdict = if r.flatness > MAX_TONAL_FLATNESS {
                "DISQUALIFYING - the conjunction reaches this frame"
            } else {
                "reported, not disqualifying - term one clears it"
            };
            println!(
                "          in the gap  {:<22} {}={:.4}  flat={:.4}  {verdict}",
                r.name,
                cand.name,
                r.values.get(ci).copied().unwrap_or(f32::NAN),
                r.flatness,
            );
        }

        // (3) A threshold that convicts the blot, named for what it is.
        //
        // Half the sparsest legitimate content — the ceremony every other
        // constant in this file follows (ADR-0071) — is not available here: the
        // conditional population has one legitimate member and it is the preset
        // being admitted, so deriving from it would be circular. The number is a
        // measurement taken between the two frozen fixtures, and the spread
        // beside it is what says whether the separation sits outside the
        // library's own range or inside it.
        let threshold = (lo + hi) / 2.0;
        let legit: Vec<f32> = rows
            .iter()
            .filter(|r| r.is_blot.is_none())
            .filter_map(|r| r.values.get(ci).copied())
            .collect();
        let spread_lo = legit.iter().copied().fold(f32::INFINITY, f32::min);
        let spread_hi = legit.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        println!("      (3) threshold {threshold:.4}, measured between the two frozen fixtures:");
        println!("          {} reads {lo:.4} - CONVICTED", blot.name);
        println!("          library spread on this column: {spread_lo:.4}..{spread_hi:.4}");
        let superseded = spread_lo / 2.0;
        let would = if lo < superseded {
            "convict"
        } else {
            "NOT convict"
        };
        println!("          (superseded ceremony: half the sparsest legitimate = {superseded:.4},");
        println!("          which would {would} the blot)");

        if convictable == 0 {
            println!("      PASSES the stop condition");
        } else {
            println!("      FAILS the stop condition");
        }
    }
}
