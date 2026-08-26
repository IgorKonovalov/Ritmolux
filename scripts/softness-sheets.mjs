#!/usr/bin/env node
// Render the artifacts Plan 0114 Phase 4 judges: every shipped line preset at a
// spread of `softness`, at the resolution the app is judged in and at one
// non-16:9 size.
//
// Rationale: ADR-0124 makes the stroke profile authorable and deliberately does
// NOT choose its default, because the profile is a claim about what an eye does
// and no test in this repo settles it. Phase 4 is the instrument. Its subjects
// are the **shipped presets** rather than a synthetic figure, so the gate is a
// judgement about the library.
//
// Usage:  node scripts/softness-sheets.mjs [out-dir]
// Writes  <out-dir>/<preset>-<size>.png            the four-up contact sheet
//         <out-dir>/<preset>-<size>-s<NNN>.png     each panel at full size
//         <out-dir>/index.md
// Default out-dir: target/softness-sheets  (gitignored, and never committed)
//
// It builds nothing itself: it writes one scratch preset per (preset, softness)
// into a temp directory and runs the `shot` example over it, which is the
// tooling the repo already trusts for labeled grids (scripts/tuple-sheets.mjs,
// Plan 0079 Phase 3 — the same shape of `human` curation gate).
//
// **Two artifacts per (preset, size), and that is not redundancy.** `shot --all`
// resizes every capture to a 320 px thumbnail, so a 1080p sheet is a 6:1
// downsample and a 4 px stroke lands at 0.7 px. That is enough to rank the four
// and to see whether the FIGURE still reads; it is not enough to judge a
// one-pixel edge. The full-size panels are what the profile is judged on, and
// the sheet is how you pick which of them to open.
//
// The stimulus is held constant across every panel, so the only thing that
// differs within a sheet is `softness`. It deliberately avoids the calibration
// traps docs/capturing.md names — no held `beat`, no held `onset` — because a
// figure latched into an edge-triggered state is not what the library looks
// like.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/// The line presets that ship today. `table` is the params table `softness`
/// belongs in — `fragment_vitrail` strokes through a parametric-curve **layer**,
/// so its top-level `[params]` is the fragment field's and has no such name.
const SUBJECTS = [
  { file: "curve_nightbloom", table: "[params]", note: "a Maurer rose, the figure the verdict was given on" },
  { file: "curve_ionwake", table: "[params]", note: "a curve at the thin end of the shipped range" },
  { file: "lsystem_vellum", table: "[params]", note: "straight stems and a branching figure" },
  { file: "star_rosewindow", table: "[params]", note: "a ring of ARCS — the primitive that surfaced this" },
  { file: "spectrum_halo", table: "[params]", note: "straight bars, where a plateau could read heavy rather than crisp" },
  { file: "fragment_vitrail", table: "[layer.params]", note: "a line layer over a fragment field" },
];

/// Both ends of the range and two values between. `1.0` is the stroke the
/// library ships today, byte for byte.
const SOFTNESS = [1.0, 0.5, 0.25, 0.0];

/// The size the app is judged at, and one non-16:9 target. The edge is specified
/// in pixels of the render target, so its **share** of the stroke differs
/// between these two even though its width does not — which is a thing to look
/// at rather than a thing to correct.
const SIZES = [
  { w: 1920, h: 1080, tag: "1080p" },
  { w: 1280, h: 800, tag: "1280x800" },
];

/// A held stimulus, identical on every panel. Moderate rather than loud: the
/// question is what the stroke looks like, and a figure pinned at full scale by
/// `bass=1` answers a different one.
const STIMULUS = "bass=0.55,mid=0.45,treb=0.4,tempo=120";

/// Enough frames for a smoothed parameter to settle and a `draw_progress` reveal
/// to finish.
const FRAMES = "240";

/// Uppercased by the renderer; its glyph table covers A-Z 0-9 space - and dot,
/// and the label is drawn at a fixed 12 px advance across a 320 px thumbnail.
const label = (softness) => `SOFTNESS ${softness.toFixed(2)}`;
const slug = (softness) => `s${String(Math.round(softness * 100)).padStart(3, "0")}`;

/// `src` with its name replaced by the panel label and `softness` bound in
/// `table`. Any existing binding is stripped first, so re-running this against a
/// preset Phase 6 has already retuned does not leave two.
///
/// Newlines are normalized to LF before anything else: the shipped presets are
/// checked out CRLF on Windows, and a table header regex that does not account
/// for the `\r` silently finds no table.
function variant(src, table, softness) {
  const stripped = src.replace(/\r\n/g, "\n").replace(/^softness\s*=.*\n/gm, "");
  const named = stripped.replace(/^name\s*=\s*".*"$/m, `name = "${label(softness)}"`);
  if (named === stripped) throw new Error("no top-level name to replace");
  const header = new RegExp(`^${table.replace(/[.[\]]/g, "\\$&")}$`, "m");
  if (!header.test(named)) throw new Error(`no ${table} table`);
  return named.replace(header, `${table}\nsoftness = "${softness}"`);
}

function shot(extra) {
  execFileSync(
    "cargo",
    [
      "run", "--release", "-p", "standalone", "--example", "shot", "--",
      "--set", STIMULUS,
      "--frames", FRAMES,
      ...extra,
    ],
    { stdio: "inherit" },
  );
}

/// A PNG of a frame that drew nothing compresses to a few KB where a drawn
/// figure is hundreds. Not a test — a number in the index, so a panel that came
/// back empty is visible without opening it.
const weight = (path) => `${Math.round(statSync(path).size / 1024)} KB`;

const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

const outDir = resolve(process.argv[2] ?? "target/softness-sheets");
mkdirSync(outDir, { recursive: true });

const index = [
  "# `softness` candidate sheets (Plan 0114 Phase 3)",
  "",
  "The artifacts Phase 4 judges. Every panel in a sheet is the same preset, the",
  "same held stimulus and the same frame — only `softness` differs. `1.00` is the",
  "stroke the library ships today, byte for byte; `0.00` is a solid stroke with a",
  "one-pixel antialiased edge.",
  "",
  "**The contact sheet ranks, the full-size panels decide.** `shot --all` resizes",
  "every capture to a 320 px thumbnail, so at 1080p a sheet is a 6:1 downsample:",
  "enough to see which panels are worth opening and whether the figure still",
  "reads, not enough to judge a one-pixel edge.",
  "",
  "Phase 4 owes three answers: a `softness` default as a number; whether a crisper",
  "stroke reads **brighter** at the same `brightness`, and roughly by how much; and",
  "whether any preset here wants to keep `1.00`, which is a legitimate outcome and",
  "the reason the parameter is authorable rather than a constant.",
  "",
  `Stimulus \`${STIMULUS}\`, ${FRAMES} frames. Generated by \`node scripts/softness-sheets.mjs\`.`,
  "",
];

for (const subject of SUBJECTS) {
  const src = readFileSync(join("presets", `${subject.file}.toml`), "utf8");
  const dir = mkdtempSync(join(tmpdir(), `lmv-softness-${subject.file}-`));
  index.push(`## ${subject.file}`, "", `${subject.note}.`, "");
  try {
    for (const softness of SOFTNESS) {
      writeFileSync(join(dir, `${slug(softness)}.toml`), variant(src, subject.table, softness));
    }
    for (const size of SIZES) {
      const geometry = `${size.w}x${size.h}`;
      const sheet = join(outDir, `${subject.file}-${size.tag}.png`);
      console.log(`\n=== ${subject.file} at ${geometry}: contact sheet ===`);
      shot(["--presets", dir, "--all", "--out", sheet, "--size", geometry]);

      const panels = [];
      for (const softness of SOFTNESS) {
        const panel = join(outDir, `${subject.file}-${size.tag}-${slug(softness)}.png`);
        console.log(`\n=== ${subject.file} at ${geometry}: ${label(softness)} ===`);
        shot(["--presets", dir, "--preset", label(softness), "--out", panel, "--size", geometry]);
        panels.push({ softness, panel, hash: digest(panel) });
      }

      // **Which panels are the same picture.** Below about two pixels of stroke
      // the edge term reaches its cap and every `softness` draws the identical
      // frame — the limit ADR-0124's Negative section states. A gate shown two
      // identical panels under different labels would read that as "the
      // parameter does nothing", so the table says it outright.
      const identical = panels.map(
        (p) => panels.find((q) => q.hash === p.hash) ?? p,
      );
      const collapsed = new Set(panels.map((p) => p.hash)).size < panels.length;

      index.push(
        `### ${geometry}`,
        "",
        `![${subject.file} ${size.tag}](${subject.file}-${size.tag}.png)`,
        "",
        "| `softness` | full-size panel | weight | reads as |",
        "|---|---|---|---|",
        ...panels.map(({ softness, panel }, i) => {
          const name = panel.split(/[\\/]/).pop();
          const twin = identical[i];
          const reads =
            twin.softness === softness
              ? "distinct"
              : `**identical to \`${twin.softness.toFixed(2)}\`**`;
          const ships = softness === 1 ? " (ships today)" : "";
          return `| \`${softness.toFixed(2)}\`${ships} | [\`${name}\`](${name}) | ${weight(panel)} | ${reads} |`;
        }),
        "",
      );
      if (collapsed) {
        index.push(
          "> **The bottom of the range collapses on this stroke.** The edge is",
          "> floored at one pixel of the render target, so once `softness` asks for",
          "> a ramp narrower than that floor, every lower value draws the identical",
          "> frame. That is the limit ADR-0124 states rather than a rendering",
          "> fault, and `thickness` is what widens the usable range — a finding",
          "> about this preset, not about the default.",
          "",
        );
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

writeFileSync(join(outDir, "index.md"), index.join("\n"));
console.log(`\nsheets, panels + index in ${outDir}`);
