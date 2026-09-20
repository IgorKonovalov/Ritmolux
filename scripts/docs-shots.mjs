#!/usr/bin/env node
// Render every committed documentation image, from the manifest below.
//
// Usage:  node scripts/docs-shots.mjs [name ...]
// Writes: docs/images/**.png  — and nothing anywhere else.
//
// With no name it renders the whole manifest. A name selects the entries it
// matches and leaves every other committed image untouched, which is what makes
// re-shooting one picture possible at all: renders are not byte-reproducible
// across machines (see below), so a whole-manifest run to refresh one card
// produces a diff over a hundred unrelated images recording driver drift.
//
// A NAME IS THE ONLY THING THE COMMAND LINE TAKES — no settings, no options, no
// environment. Every capture setting stays in the manifest entry, so a re-render
// cannot land at a size or tier nobody wrote down, and an image set nobody can
// re-shoot without remembering a command line is an image set that goes stale.
// `git status` is clean after a re-run on the same machine and binary, so "are
// these current" is answerable by running this and looking.
//
// THE MANIFEST IS THE PROVENANCE RECORD. Every committed PNG under docs/images/
// has exactly one entry here, and the entry carries the whole command: preset
// file, stimulus, clip length, hop, size and tier. There is no way to ask "how
// was this made" and not get an answer, and swapping which preset represents a
// family is one line plus a re-run (ADR-0100).
//
// ---------------------------------------------------------------------------
// THIS IS NOT A CI GATE, AND MUST NOT BECOME ONE.
//
// The obvious next thought — "run it in CI and fail if the images changed" — does
// not work here and adding it would produce a permanently red build. Renders are
// not byte-reproducible across machines: the golden suite treats a 0.02 mean
// channel difference as ordinary rasterizer drift, and eight of its twenty
// baselines rewrite on a clean bless on the dev box alone. A different GPU, a
// different driver or a WARP fallback moves pixels for reasons that have nothing
// to do with whether the documentation is true.
//
// So freshness is a human duty at a named cadence (the close-ceremony
// operator-doc sweep), not a check. Re-run this when the thing a picture depicts
// changes.
// ---------------------------------------------------------------------------
//
// Why these capture settings, since they are the same on every entry and could
// look arbitrary:
//
//   --signal dynamic:110   the only synthesized kind that rises and falls through
//                          the real analyzer. Every other kind is a steady tone,
//                          and `--set` cannot reach the 64-band array at all, so
//                          `bin(x)` reads 0 and a spectrum preset photographs
//                          inert (docs/capturing.md, "the three calibration
//                          traps").
//   --frame-at 300         the last hop of the loudest beat — see below.
//   --tier rich            what the app starts on. A README picture should be
//                          what a user sees, and a Rich capture is not measurably
//                          larger on disk than a Floor one.
//
// WHY HOP 300 AND NOT 340. Plan 0088 specifies hop 340, on the reasoning that it
// is ~3.8 s of scene time — nearly twice a default `--frames 120` capture — and
// that accumulating families need the extra development. The first half of that
// is right and the conclusion is not, because of where 340 lands in the clip.
//
// `dynamic_groove` is an 8-beat phrase that builds geometrically for six beats
// and then RESTS for two at an amplitude of 0.04. At 110 BPM a beat is 0.545 s
// and a hop is 512 samples at 48 kHz, so:
//
//   beat 5 (the loudest, amp 0.968)   2.73-3.27 s   hops 255-306
//   beat 6 (the rest,   amp 0.040)    3.27-3.82 s   hops 306-357
//   beat 7 (the rest,   amp 0.040)    3.82-4.36 s   hops 357-409, clipped at 375
//
// So hop 340 is not the peak of the build — it is 34 hops INTO the quiet bar,
// and every reactive family photographs there at its resting state. Measured on
// the shipped library: spectrum_halo's readout is collapsed to a stub at 340 and
// fully extended at 300; fragment_supernova's `kaleido_order` select drops to its
// lowest arm and the frame flattens to a near-uniform wash.
//
// Hop 300 is the last hop of beat 5: maximum energy, and the most scene time any
// accumulating family can have before the rest. attractor_leviathan — the
// preset the plan measured — is a fuller rosette at 300 than at 340, so the
// accumulation argument does not cost anything by moving.
//
// WHY AN ACCUMULATING FAMILY'S CARD IS HOP 2754 AND THE CLIP IS 30 s. The
// paragraph above buys the loudest hop the clip has; what it cannot buy is a
// world that is not there yet at 3.2 s. A feedback field, a reaction-diffusion
// field, a cellular field, a trail-fed attractor and a particle population all
// keep developing for tens of seconds — warp_ladder's own header records
// coverage 0.408 at its 30 s row against 0.619 at 300 s — so their cards
// photograph a world mid-assembly (ADR-0235).
//
// The phrase is what makes a late hop a loud one. `dynamic_groove` repeats its
// 8-beat phrase for as long as the clip runs, so the same position in a later
// phrase carries the same amplitude:
//
//   a beat    0.545 s   26,182 samples   51.14 hops
//   a phrase  4.36 s   209,456 samples  409.09 hops
//
// Hop 300 sits 0.87 of the way into beat 5 of phrase 0 — the loudest beat, amp
// 0.968. Six phrases later that same point is sample 153,600 + 6x209,456 =
// 1,410,336, which is hop 2754: beat 53, and 53 = 8x6 + 5, so it is beat 5 of
// its phrase at the same amplitude and the same position inside it. That is
// 29.4 s of scene time, the neighbourhood of the 30 s render backlog 0254 read
// warp_tracery's card against.
//
// The swarm family's hop is 2828 rather than 2754, and for the reason its own
// per-preset entries already record: a swarm is a MOTION and photographs as
// uniform noise at full energy, so it wants the phrase's quiet bar. Hop 374 is
// 0.31 into beat 7, the second resting beat; six phrases later that is hop 2828,
// beat 55 = 8x6 + 7.
//
// Neither hop exists in the clip `shot` synthesizes by default: 4 s is 375
// analysis hops, and a `--frame-at` past the last one fails the run rather than
// clamping. So an entry whose hop needs a longer clip carries `signalSecs`, and
// that is the whole of what `--signal-secs` is for (Plan 0210). Lengthening
// appends phrases rather than re-timing the ones already there, so the hops a
// 4 s clip had are the same samples in a 30 s one — which is why a card whose
// hop did not move does not move either.

import { execFileSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { basename, relative, resolve, sep } from "node:path";

// ---------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------

/// Every field is spelled out on every entry rather than defaulted, so one entry
/// read in isolation reconstructs its whole command.
///
/// `hop` is 300 unless a note says otherwise. Where it differs, the note says
/// what was wrong with 300 for that preset — the hop is a judgement about one
/// picture, and an unexplained number would look like a typo.
///
/// The gallery holds exactly one entry per system name in `SystemKind::from_name`
/// and is filed under that name. That correspondence is asserted in
/// `core/tests/suite/hygiene.rs`, not here — see the block below the manifest.
///
/// **EVERY FAMILY NOW SHIPS SEVERAL PRESETS, so every slot below is a choice.**
/// Nine were judged at the Plan 0088 close (Phase 7) and carry the verdict where
/// one pick displaced another; four of those nine were chosen when the family
/// held exactly one preset, so they were never a comparison and are marked
/// UNJUDGED. The three newest families were picked by `dev`; `warp_mesh` was
/// judged and kept at Plan 0136 Phase 10, and the other two were seen at that
/// same look and left standing. A swap is one line here plus a re-run.
///
/// Two slots came out of the Plan 0088 pass with a picture that is accepted
/// rather than good, and the fix is content work rather than a hop:
/// `emitter_perseids` bunches its fan into the right half at every hop tried,
/// and `star_rosewindow`'s outermost ring runs off all four edges. Both are
/// recorded as content-lane notes at that close, not as manifest bugs.
// --- the per-preset gallery cards ---------------------------------------
//
// One card per SHIPPED preset, filed under docs/images/gallery/presets/. This
// is a SECOND collection and not a replacement: the entries above hold exactly
// one picture per system and are the ones the README and the guide print, while
// these are the site's exhaustive gallery, which a markdown file has no room for
// and a page does.
//
// THE LIST IS SPELLED OUT rather than globbed from presets/, and that is the
// whole mechanism. A glob could never report a preset with no card, because a
// preset with no card would simply not be in it. `core/tests/suite/hygiene.rs`,
// `every_shipped_preset_has_a_gallery_card`, reads the names below and the
// contents of presets/ and fails when they disagree in either direction --
// the same shape as `every_system_has_a_gallery_image` above it.
//
// The capture settings are uniform WITHIN A FAMILY and the hop is not uniform
// across them. These cards are read as a grid, against each other, so a
// PER-PRESET hop would be tuning one thumbnail at the cost of the comparison —
// but a family that is still assembling its world at 3.2 s photographs as a
// picture of the assembly, which costs the comparison far more (ADR-0235). So
// the hop resolves per family, and `CARD_HOP_OVERRIDES` above stays for the
// per-preset exception and still beats the family's own number.
//
// 640x360 rather than the 1280x720 above. A card is displayed at roughly a
// third of a page column, and over a hundred of them at full size would put tens of
// megabytes of derived PNG into the repository for detail no reader can see.
const CARD_SIZE = "640x360";
const CARD_HOP_OVERRIDES = { swarm_braid: 374, swarm_drift: 374, swarm_shatter: 374, swarm_stipple: 374 };

/// The clip `shot` synthesizes when `--signal-secs` is absent — `SIGNAL_SECS` in
/// standalone/src/shot/args.rs — and the analysis-hop arithmetic every `hop`
/// here is an index into. `HOP` is `rlx_core::dsp::HOP_SIZE`; `RATE` is what
/// `synth_signal_secs` builds every kind at.
const DEFAULT_SIGNAL_SECS = 4;
const SIGNAL_RATE = 48_000;
const SIGNAL_HOP = 512;

/// Analysis hops a clip of `secs` yields — the same floor division `total_hops`
/// does in standalone/src/shot/film.rs, so an entry this file accepts is an
/// entry `check_hops` accepts.
const totalHops = (secs) => Math.floor(Math.round(secs * SIGNAL_RATE) / SIGNAL_HOP);

/// Seconds of signal `hop` needs to exist at all, or `undefined` when the CLI's
/// own default clip already reaches it.
///
/// `undefined` is not a missing setting. It is what keeps `--signal-secs` OFF
/// the command line, so an entry that does not carry one is rendered by exactly
/// the command that produced its committed PNG, down to the flag roster.
const signalSecsFor = (hop) => {
  const needed = Math.ceil(((hop + 1) * SIGNAL_HOP) / SIGNAL_RATE);
  return needed > DEFAULT_SIGNAL_SECS ? needed : undefined;
};

/// A card's hop when the roster above does not name its preset, keyed by the
/// name prefix a family's worlds share (`reaction_*` is `reaction_diffusion`,
/// `warp_*` is `warp_mesh`, and so on). A prefix with no row here falls to
/// [`CARD_HOP`] — the family redraws its picture from the frame it is given and
/// has nothing to develop.
///
/// **The family is the grain, not the preset** (ADR-0235): accumulation is a
/// property of the system and its feedback configuration, which is how a
/// per-preset roster grew to four entries covering one family and stopped. The
/// header above derives both numbers from the phrase.
const CARD_HOP = 300;
const CARD_HOP_DEVELOPED = 2754;
const CARD_HOP_DEVELOPED_IN_THE_LULL = 2828;
const CARD_FAMILY_HOPS = {
  attractor: CARD_HOP_DEVELOPED,
  cellular: CARD_HOP_DEVELOPED,
  emitter: CARD_HOP_DEVELOPED,
  reaction: CARD_HOP_DEVELOPED,
  swarm: CARD_HOP_DEVELOPED_IN_THE_LULL,
  warp: CARD_HOP_DEVELOPED,
};

/// Highest precedence first: the per-preset roster, the preset's family, 300.
const cardHop = (preset) =>
  CARD_HOP_OVERRIDES[preset] ?? CARD_FAMILY_HOPS[preset.split("_")[0]] ?? CARD_HOP;

/// Every shipped preset, grouped by the system it draws with. The comment on
/// each group is a count, so a family that gains a preset and not a card is
/// visible here as well as in the test.
const CARDS = [
  // analytic_field (12)
  "analytic_echoplate",
  "analytic_juliacircuit",
  "analytic_lacegrid",
  "analytic_multibrot",
  "analytic_parabolicdust",
  "analytic_pearlstring",
  "analytic_ringorbit",
  "analytic_seahorse",
  "analytic_searchlight",
  "analytic_stainedglass",
  "analytic_standingwave",
  "analytic_twobandjulia",
  // attractor (20)
  "attractor_clifford",
  "attractor_cliffordgallery",
  "attractor_dejonggallery",
  "attractor_dragon",
  "attractor_fern",
  "attractor_fernmono",
  "attractor_ink",
  "attractor_leviathan",
  "attractor_lorenzgallery",
  "attractor_lorenzknot",
  "attractor_thomas",
  "attractor_thomasgallery",
  "attractor_thomasred",
  "attractor_torusknot",
  "attractor_valentine",
  "attractor_volute",
  "attractor_walkdejong",
  "attractor_walkknot",
  "attractor_walkrho",
  "attractor_walkthomas",
  // cellular (5)
  "cellular_ember_life",
  "cellular_labyrinth",
  "cellular_spiral_bloom",
  "cellular_tide_bugs",
  "cellular_wavefront",
  // emitter (5)
  "emitter_driftfield",
  "emitter_emberjet",
  "emitter_heartfall",
  "emitter_perseids",
  "emitter_petalfall",
  // fragment_field (14)
  "fragment_driftmono",
  "fragment_drostemono",
  "fragment_etchingplate",
  "fragment_interferencemono",
  "fragment_mandala",
  "fragment_nebula",
  "fragment_strata",
  "fragment_sumi",
  "fragment_supernova",
  "fragment_tiled",
  "fragment_tiledmono",
  "fragment_tunnel",
  "fragment_vitrail",
  "fragment_whorl",
  // lsystem (6)
  "lsystem_bower",
  "lsystem_coral",
  "lsystem_icecrystal",
  "lsystem_rime",
  "lsystem_sumimono",
  "lsystem_vellum",
  // parametric_curve (13)
  "curve_blueprint",
  "curve_broadside",
  "curve_cogwheel",
  "curve_gyre",
  "curve_inkpendulum",
  "curve_ionwake",
  "curve_lacework",
  "curve_loom",
  "curve_nightbloom",
  "curve_phosphor",
  "curve_prismscope",
  "curve_rosemono",
  "curve_turnabout",
  // reaction_diffusion (7)
  "reaction_etching",
  "reaction_fluxmono",
  "reaction_glaciermono",
  "reaction_lichen",
  "reaction_mitosis",
  "reaction_spotmono",
  "reaction_verdigris",
  // shape_collage (4)
  "collage_mono",
  "collage_nocturne",
  "collage_onwhite",
  "collage_suprematist",
  // shape_field (9)
  "shape_aperture",
  "shape_contourmono",
  "shape_facet",
  "shape_heartmono",
  "shape_lion",
  "shape_maple",
  "shape_pulse",
  "shape_ringmono",
  "shape_strataheart",
  // spectrum (5)
  "spectrum_anemone",
  "spectrum_metermono",
  "spectrum_radialbloom",
  "spectrum_ridge",
  "spectrum_skyline",
  // star_pattern (4)
  "star_corona",
  "star_mandala_bordered",
  "star_rosewindow",
  "star_zellij",
  // swarm (5)
  "swarm_braid",
  "swarm_drift",
  "swarm_murmuration",
  "swarm_shatter",
  "swarm_stipple",
  // warp_mesh (7)
  "warp_cauldron",
  "warp_ladder",
  "warp_millrace",
  "warp_sirocco",
  "warp_smoke",
  "warp_tracery",
  "warp_wellhead",
];

const IMAGES = [
  {
    // Judged at the Plan 0088 close (Phase 7) against fragment_supernova,
    // fragment_vitrail, fragment_mandala and attractor_volute. Supernova held
    // this slot through Phases 2-6 and lost it on the front page's terms rather
    // than on its own: a flat salmon field over most of the frame reads as
    // wallpaper at the top of a README. Tunnel has real blacks, so it carries
    // contrast and depth at any column width.
    out: "docs/images/hero.png",
    presetFile: "presets/fragment_tunnel.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },

  {
    // The guide's opening figure: the smallest preset that is worth looking at.
    // Rendered from the same file the guide prints, so the picture cannot drift
    // from the listing beside it.
    out: "docs/images/preset-minimal.png",
    presetFile: "docs/examples/minimal.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },

  // --- the tuning walkthrough: five steps, one hop -------------------------
  //
  // NOT hop 300. These five are read against each other, so they share a hop —
  // and it has to be a hop where the music is at an ORDINARY level, not at the
  // top of the build. Step 2's whole lesson is that its bindings are dead
  // wherever real material actually sits; captured at 300, the loudest hop in
  // the clip, its thresholds all fire and the picture would contradict the
  // report row printed beside it. Hop 230 is inside beat 4, mid-build.
  ...[
    ["step-1-constants", "1"],
    ["step-2-naive-bands", "2"],
    ["step-3-calibrated", "3"],
    ["step-4-eased", "4"],
    ["step-5-colour-and-beat", "5"],
  ].map(([file, n]) => ({
    out: `docs/images/walkthrough/step-${n}.png`,
    presetFile: `docs/examples/tuning/${file}.toml`,
    signal: "dynamic:110",
    hop: 230,
    size: "1280x720",
    tier: "rich",
  })),

  // --- the curve families: one per `[curve] family` beyond the rose --------
  //
  // The guide's `parametric_curve` section pictures the rose through the
  // gallery entry below; these four picture the other families. No shipped
  // preset draws them yet, so each renders a teaching preset from
  // docs/examples/curves/ - the guide prints the file name under the picture,
  // and the file is the whole recipe. Filed OUTSIDE docs/images/gallery/, whose
  // flat stems `core/tests/suite/hygiene.rs` reads as system names.
  ...["lissajous", "hypotrochoid", "superformula", "harmonograph"].map((family) => ({
    out: `docs/images/curves/${family}.png`,
    presetFile: `docs/examples/curves/${family}.toml`,
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  })),

  // --- the analytic field's families beyond the plate ----------------------
  //
  // The gallery entry below pictures `chladni`; this pictures `escape_time`,
  // from a teaching preset for the curves' reason - no shipped preset draws
  // it yet. Filed OUTSIDE docs/images/gallery/ for the same reason too.
  {
    out: "docs/images/field/escape_time.png",
    presetFile: "docs/examples/field/escape_time.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },

  // --- the cellular system's families beyond life_like --------------------
  //
  // The gallery entry below pictures `life_like`; these picture the other two
  // families, each from the one shipped world that draws it. Filed outside the
  // gallery for the curves' reason.
  {
    out: "docs/images/cellular/larger_than_life.png",
    presetFile: "presets/cellular_tide_bugs.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    out: "docs/images/cellular/cyclic.png",
    presetFile: "presets/cellular_spiral_bloom.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },

  // --- the gallery: one per SystemKind ------------------------------------

  {
    // fragment_field — provisional, from 8 candidates.
    out: "docs/images/gallery/fragment_field.png",
    presetFile: "presets/fragment_whorl.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // swarm — judged at the Plan 0088 close (Phase 7). swarm_drift held this
    // slot through Phase 3 and `dev` flagged it as the weakest picture in the
    // set; the close agreed, for a reason no column measures: drift is charcoal
    // on black and collapses to a dark rectangle at README column width. Both
    // swarm presets were shot at 300 and 374 and shatter won at both.
    //
    // The hop is still NOT 300, and for drift's original reason: a swarm is a
    // MOTION, and at full energy it photographs as uniform noise. 374 is inside
    // the phrase's quiet bar, where the flock settles onto the flow field and
    // the wave crest driving it becomes legible.
    out: "docs/images/gallery/swarm.png",
    presetFile: "presets/swarm_shatter.toml",
    hop: 374,
    signal: "dynamic:110",
    size: "1280x720",
    tier: "rich",
  },
  {
    // parametric_curve — provisional, from 2 candidates.
    out: "docs/images/gallery/parametric_curve.png",
    presetFile: "presets/curve_nightbloom.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // lsystem — UNJUDGED. Chosen when this family shipped one preset; 5 ship
    // now and none has been compared against vellum.
    out: "docs/images/gallery/lsystem.png",
    presetFile: "presets/lsystem_vellum.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // star_pattern — UNJUDGED. Chosen when this family shipped one preset; 4
    // ship now. The Plan 0088 close notes this picture's outermost ring runs off
    // all four edges, and called that content work rather than a hop.
    out: "docs/images/gallery/star_pattern.png",
    presetFile: "presets/star_rosewindow.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // reaction_diffusion — provisional, from 3 candidates.
    out: "docs/images/gallery/reaction_diffusion.png",
    presetFile: "presets/reaction_verdigris.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // attractor — provisional, from 17 candidates.
    out: "docs/images/gallery/attractor.png",
    presetFile: "presets/attractor_leviathan.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // spectrum — UNJUDGED. `spectrum_radialbloom` took this slot when the owner
    // retired `spectrum_halo` in its favour, so it inherits the slot rather than
    // winning it; the other four spectrum worlds have not been compared at it.
    out: "docs/images/gallery/spectrum.png",
    presetFile: "presets/spectrum_radialbloom.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // shape_field — the first shipped world on the system (ADR-0105), and the
    // one that shows what the family is FOR: the scene hands the palette a
    // FIGURE COORDINATE rather than a level, so `palette_steps` turns it into
    // flat graphic bands and `palette_contour` draws the hairline between them.
    // Chosen over shape_aperture and shape_facet on that ground; the three mono
    // worlds are deliberate two-ink prints and read as a different family at
    // gallery size.
    out: "docs/images/gallery/shape_field.png",
    presetFile: "presets/shape_pulse.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // warp_mesh — JUDGED at Plan 0136 Phase 10 and kept. Backlog 0133 and that
    // plan both record that this family ships no preset, which had stopped being
    // true: four warp worlds ship, and all four were shot at this hop and
    // compared. A fifth, `warp_smoke`, was authored during that same phase and
    // was NOT taken for this slot - the plume is a stronger picture of smoke than
    // wellhead is of the family.
    //
    // The whole family is SOFT — a warp field has no edges of its own, it only
    // moves what is already there — so the question is which world still has a
    // readable subject at gallery size. Wellhead does: a dark star-shaped
    // aperture against teal, with the ring feeding it legible as depth. Cauldron
    // held this slot first and lost it on that ground, being a symmetric bloom
    // with no hard edge anywhere in it; millrace is one crescent on black, and
    // sirocco is a horizontal drape that reads as an abstract gradient.
    //
    // Confirmed at that phase's own look. A later swap is one line here plus a
    // re-run.
    out: "docs/images/gallery/warp_mesh.png",
    presetFile: "presets/warp_wellhead.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // shape_collage — the first shipped world on the system (ADR-0123), and the
    // only family here that draws a GRAPHIC rather than light: a pixel starts at
    // the paper colour and composites each element with `over`, so the array
    // index is the depth. Chosen over the three others for the same reason
    // shape_pulse was — collage_mono and collage_onwhite are two-ink by intent.
    out: "docs/images/gallery/shape_collage.png",
    presetFile: "presets/collage_suprematist.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // emitter — UNJUDGED. Chosen when this family shipped one preset; 5 ship
    // now. The Plan 0088 close notes this picture bunches its fan into the right
    // half at every hop tried, and called that content work rather than a hop.
    out: "docs/images/gallery/emitter.png",
    presetFile: "presets/emitter_perseids.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // analytic_field — UNJUDGED. `analytic_standingwave` was the owner's pick
    // when the system's first shipped worlds landed: the Chladni family, which
    // is what the slot's teaching preset showed before it. The escape-time
    // worlds have not been compared at it.
    out: "docs/images/gallery/analytic_field.png",
    presetFile: "presets/analytic_standingwave.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // cellular — UNJUDGED, and not a choice: `cellular_ember_life` is the one
    // shipped world on the life_like family, the family the slot pictured
    // before it through a teaching preset.
    out: "docs/images/gallery/cellular.png",
    presetFile: "presets/cellular_ember_life.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },

  // --- the gallery: one card per shipped preset ---------------------------
  //
  // The only entries whose settings are computed rather than written out, and
  // the two computations are the ones a hand-written number would get wrong:
  // the hop follows the family, and the clip is whatever that hop needs to
  // exist in.
  ...CARDS.map((preset) => {
    const hop = cardHop(preset);
    return {
      out: `docs/images/gallery/presets/${preset}.png`,
      presetFile: `presets/${preset}.toml`,
      signal: "dynamic:110",
      signalSecs: signalSecsFor(hop),
      hop,
      size: CARD_SIZE,
      tier: "rich",
    };
  }),
];

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// The one directory this script is allowed to write into. A manifest entry
/// pointing anywhere else is a bug, and one that overwrote a source file would
/// be an expensive one to notice.
const IMAGE_ROOT = resolve("docs/images");

const REQUIRED = ["out", "presetFile", "signal", "hop", "size", "tier"];

/// Validate one entry before spawning anything. A bad entry names itself.
function check(entry, index) {
  const where = `manifest entry ${index} (${entry.out ?? "no `out`"})`;
  for (const key of REQUIRED) {
    if (entry[key] === undefined || entry[key] === "") {
      throw new Error(`${where}: missing \`${key}\``);
    }
  }
  const out = resolve(entry.out);
  if (out !== IMAGE_ROOT && !out.startsWith(IMAGE_ROOT + sep)) {
    throw new Error(
      `${where}: writes outside docs/images/ (${relative(".", out)}) — refusing`,
    );
  }
  if (!out.endsWith(".png")) {
    throw new Error(`${where}: \`out\` must be a .png path`);
  }
  if (!existsSync(entry.presetFile)) {
    throw new Error(`${where}: no preset file at ${entry.presetFile}`);
  }
  if (!Number.isInteger(entry.hop) || entry.hop < 0) {
    throw new Error(`${where}: \`hop\` must be a whole analysis-hop index`);
  }
  // `signalSecs` is the one optional field, and absent means "no --signal-secs
  // on the command line" rather than "not written down" — see `signalSecsFor`.
  if (entry.signalSecs !== undefined) {
    if (typeof entry.signalSecs !== "number" || !(entry.signalSecs > 0)) {
      throw new Error(`${where}: \`signalSecs\` must be a positive number of seconds`);
    }
  }
  // The hop has to exist in the clip this entry asks for. `shot` checks the same
  // thing and fails the run, but it does so after a renderer is up and a
  // thousand hops have been advanced; here it costs nothing and names the entry.
  const secs = entry.signalSecs ?? DEFAULT_SIGNAL_SECS;
  const hops = totalHops(secs);
  if (entry.hop >= hops) {
    throw new Error(
      `${where}: hop ${entry.hop} is past the end of a ${secs}s clip (${hops} analysis hops)`,
    );
  }
}

IMAGES.forEach(check);

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// The last path segment without its extension: `warp_tracery` for both
/// `presets/warp_tracery.toml` and `docs/images/gallery/presets/warp_tracery.png`.
const stem = (path) => basename(path).replace(/\.[^.]*$/, "");

/// The four spellings a command-line name may use for one entry. Backslashes
/// fold to forward slashes, so a path completed by a Windows shell matches the
/// forward-slash paths the manifest is written in.
const aliases = (entry) =>
  [entry.out, entry.presetFile, stem(entry.out), stem(entry.presetFile)].map((a) =>
    a.replaceAll("\\", "/"),
  );

/// Matching is EXACT against those spellings rather than a substring test.
/// `attractor_clifford` and `attractor_cliffordgallery` are both shipped presets,
/// so a substring match would re-render a card nobody asked for and put its
/// driver drift in the same commit as the intended one.
const requested = process.argv.slice(2).map((name) => name.replaceAll("\\", "/"));
const unmatched = requested.filter(
  (name) => !IMAGES.some((entry) => aliases(entry).includes(name)),
);

// A name that matches nothing stops the run before the first render, rather
// than rendering the names that did match: a typo'd name would otherwise be
// indistinguishable from a picture that is simply already current.
if (unmatched.length > 0) {
  console.error(`no manifest entry is named: ${unmatched.join(", ")}`);
  console.error("\nthe names this manifest knows:");
  const known = [...new Set(IMAGES.flatMap((e) => [stem(e.out), stem(e.presetFile)]))].sort();
  for (const name of known) {
    console.error(`  ${name}`);
  }
  process.exit(1);
}

/// No name is every entry — the whole-manifest run the operator docs name.
const selected =
  requested.length === 0
    ? IMAGES
    : IMAGES.filter((entry) => aliases(entry).some((a) => requested.includes(a)));

// ---------------------------------------------------------------------------
// THE GALLERY-vs-`SystemKind` CROSS-CHECK IS NOT HERE. It lives in
// core/tests/suite/hygiene.rs, `every_system_has_a_gallery_image`.
//
// It used to run at the top of this file, and it was doing exactly what its
// comment said: `SystemKind::from_name` is the source of truth for what a system
// IS, so it is the source of truth for what the gallery owes a picture of, and a
// hardcoded roster would let a system ship with no picture and nothing saying so.
// It caught precisely that — and then took the whole run down with it, including
// the eight images that had nothing to do with the missing systems.
//
// The guard was never the bug; the cadence was. The check is PURE TEXT — the
// manifest and schema.rs, no GPU — and the only thing executing it was a human
// running this script at a plan close, so three systems accumulated over eleven
// days and the sweep was silently dead the whole time. Moved into a test, it
// fails on the commit that ships a scene.
//
// What did NOT move, and must not: "the images are current". Renders are not
// byte-reproducible across machines, so ADR-0100 keeps that out of CI and leaves
// it a human duty at a named cadence. Two different claims; only the mechanical
// one is gated.
// ---------------------------------------------------------------------------

// Checked up front, all of them, before the first render: a manifest typo in the
// last entry should not cost the eight renders before it.
let failures = 0;
for (const entry of selected) {
  // The manifest index rather than the position in this run, so the name a
  // failure reports is the one to go and look at in the list above.
  const where = `manifest entry ${IMAGES.indexOf(entry)} (${entry.out})`;
  console.log(
    `\n${entry.out}\n  ${entry.presetFile} @ hop ${entry.hop}, ` +
      `${entry.signal}${entry.signalSecs ? ` for ${entry.signalSecs}s` : ""}, ` +
      `${entry.size}, tier ${entry.tier}`,
  );
  try {
    execFileSync(
      "cargo",
      [
        "run", "--release", "-p", "standalone", "--example", "shot", "--",
        "--preset-file", entry.presetFile,
        "--signal", entry.signal,
        // Absent unless the entry's hop needs a longer clip than `shot`'s own
        // default, so an entry whose hop did not move is rendered by the
        // command that produced its committed PNG.
        ...(entry.signalSecs ? ["--signal-secs", String(entry.signalSecs)] : []),
        "--frame-at", String(entry.hop),
        "--size", entry.size,
        "--tier", entry.tier,
        "--out", entry.out,
      ],
      { stdio: "inherit" },
    );
  } catch (err) {
    // A non-zero `shot` — a hop past the end of the clip, an unreadable preset,
    // no GPU adapter — must not leave the previous PNG sitting there looking
    // current, because `git status` would then be clean over a stale image and
    // the run's whole freshness claim would be false. So the old file goes with
    // the failed render, and the entry is named on a non-zero exit.
    rmSync(entry.out, { force: true });
    console.error(`FAILED: ${where} (previous image removed): ${err.message}`);
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n${failures} of ${selected.length} images failed to render`);
  process.exit(1);
}
console.log(
  `\n${selected.length} of ${IMAGES.length} images written under docs/images/`,
);
