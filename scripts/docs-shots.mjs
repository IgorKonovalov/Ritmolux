#!/usr/bin/env node
// Render every committed documentation image, from the manifest below.
//
// Usage:  node scripts/docs-shots.mjs
// Writes: docs/images/**.png  — and nothing anywhere else.
//
// No arguments, no environment, no options. That is the point: an image set
// nobody can re-shoot without remembering a command line is an image set that
// goes stale. `git status` is clean after a re-run on the same machine and
// binary, so "are these current" is answerable by running this and looking.
//
// THE MANIFEST IS THE PROVENANCE RECORD. Every committed PNG under docs/images/
// has exactly one entry here, and the entry carries the whole command: preset
// file, stimulus, hop, size and tier. There is no way to ask "how was this made"
// and not get an answer, and swapping which preset represents a family is one
// line plus a re-run (ADR-0100).
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

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
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
/// and is filed under that name, so a system added later shows up as a missing
/// file rather than as a silent gap. Four families have exactly one preset and
/// choose themselves; the other five had a real choice and were **judged at the
/// Plan 0088 close** (Phase 7) — the entries below carry the verdict where one
/// pick displaced another. A later swap is one line here plus a re-run.
///
/// Two of the single-preset families came out of that pass with a picture that
/// is accepted rather than good, and the fix is content work rather than a hop:
/// `emitter_perseids` bunches its fan into the right half at every hop tried,
/// and `star_rosewindow`'s outermost ring runs off all four edges. Both are
/// recorded as content-lane notes at the Plan 0088 close, not as manifest bugs.
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
    // lsystem — the only lsystem preset.
    out: "docs/images/gallery/lsystem.png",
    presetFile: "presets/lsystem_vellum.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // star_pattern — the only star_pattern preset.
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
    // spectrum — the only spectrum preset.
    out: "docs/images/gallery/spectrum.png",
    presetFile: "presets/spectrum_halo.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
  {
    // emitter — the only emitter preset.
    out: "docs/images/gallery/emitter.png",
    presetFile: "presets/emitter_perseids.toml",
    signal: "dynamic:110",
    hop: 300,
    size: "1280x720",
    tier: "rich",
  },
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
}

IMAGES.forEach(check);

/// The system roster, read out of the Rust rather than mirrored here.
///
/// `SystemKind::from_name` is the source of truth for what a system *is*, so it
/// is also the source of truth for what the gallery owes a picture of. A
/// hardcoded list of nine names would let a tenth system ship with no gallery
/// image and nothing saying so — which is the exact silent gap this check exists
/// to turn into a loud one. Same discipline as `tuple-sheets.mjs` reading the
/// attractor roster out of `family.rs`.
function systemNames() {
  const path = "core/src/preset/schema.rs";
  const src = readFileSync(path, "utf8");
  const start = src.indexOf("pub fn from_name(name: &str) -> Option<Self> {");
  if (start < 0) throw new Error(`could not find SystemKind::from_name in ${path}`);
  const body = src.slice(start, src.indexOf("}", src.indexOf("_ => return None", start)));
  const names = [...body.matchAll(/"([a-z_]+)" => SystemKind::/g)].map((m) => m[1]);
  if (names.length === 0) throw new Error(`read no system names out of ${path}`);
  return names;
}

/// Every system has exactly one gallery image, filed under its own name.
{
  const gallery = new Set(
    IMAGES.map((e) => e.out)
      .filter((out) => out.includes("gallery/"))
      .map((out) => basename(out, ".png")),
  );
  const missing = systemNames().filter((name) => !gallery.has(name));
  const extra = [...gallery].filter((name) => !systemNames().includes(name));
  if (missing.length > 0 || extra.length > 0) {
    throw new Error(
      "the gallery does not match SystemKind::from_name" +
        (missing.length ? `\n  no image for: ${missing.join(", ")}` : "") +
        (extra.length ? `\n  not a system: ${extra.join(", ")}` : ""),
    );
  }
}

// Checked up front, all of them, before the first render: a manifest typo in the
// last entry should not cost the eight renders before it.
let failures = 0;
for (const [index, entry] of IMAGES.entries()) {
  const where = `manifest entry ${index} (${entry.out})`;
  console.log(
    `\n${entry.out}\n  ${entry.presetFile} @ hop ${entry.hop}, ` +
      `${entry.signal}, ${entry.size}, tier ${entry.tier}`,
  );
  try {
    execFileSync(
      "cargo",
      [
        "run", "--release", "-p", "standalone", "--example", "shot", "--",
        "--preset-file", entry.presetFile,
        "--signal", entry.signal,
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
  console.error(`\n${failures} of ${IMAGES.length} images failed to render`);
  process.exit(1);
}
console.log(`\n${IMAGES.length} images written under docs/images/`);
