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
//   --frame-at 340         the clip runs ~375 analysis hops, so 340 is ~3.8 s of
//                          scene time — nearly twice what a default --frames 120
//                          capture reaches. Accumulating families need it:
//                          attractor_leviathan at hop 46 is an undeveloped smudge
//                          and at hop 340 is the finished rosette.
//   --tier rich            what the app starts on. A README picture should be
//                          what a user sees, and a Rich capture is not measurably
//                          larger on disk than a Floor one.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

// ---------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------

/// Every field is spelled out on every entry rather than defaulted, so one entry
/// read in isolation reconstructs its whole command.
///
/// `hop` is 340 unless a note says otherwise. Where it differs, the note says
/// what was wrong with 340 for that preset — the hop is a judgement about one
/// picture, and an unexplained number would look like a typo.
const IMAGES = [
  {
    out: "docs/images/hero.png",
    presetFile: "presets/fragment_supernova.toml",
    signal: "dynamic:110",
    // NOT 340. The 4 s clip is a single 8-beat phrase that builds almost to its
    // end, so hop 340 is the loudest moment in it — and Supernova puts peak
    // energy into `warp`, `zoom` and `palette_mix` at once, which at the top of
    // the build flattens into a near-uniform orange wash. Hop 70 is early in the
    // same phrase, where the fold still reads as a rosette. Fragment fields do
    // not accumulate, so the "late enough to be developed" argument behind 340
    // does not apply to this entry.
    hop: 70,
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
    // current. Name the entry and keep a non-zero exit.
    console.error(`FAILED: ${where}: ${err.message}`);
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n${failures} of ${IMAGES.length} images failed to render`);
  process.exit(1);
}
console.log(`\n${IMAGES.length} images written under docs/images/`);
