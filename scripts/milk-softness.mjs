#!/usr/bin/env node
// Render the artifacts Plan 0114 Phase 8 judges: `warp_mesh`'s own line path at a
// spread of `softness`, beside the instructions for putting `foo_vis_milk2` next
// to them.
//
// Rationale: ADR-0124 pins `warp_mesh` to the pre-0114 profile rather than
// letting it follow the line default, because it answers to a different judge —
// `foo_vis_milk2`, ADR-0113's fidelity reference — and that instrument has never
// been pointed at this question. Phase 8 points it. This script is the setup, so
// that session is a judging session and not a setup session.
//
// Usage:  node scripts/milk-softness.mjs [out-dir]
// Writes  <out-dir>/<subject>.milk               the reference side's input
//         <out-dir>/<subject>.toml               the converted bundle
//         <out-dir>/<subject>-<size>-s<NNN>.png  one panel per softness
//         <out-dir>/index.md
// Default out-dir: target/milk-softness  (gitignored, and never committed)
//
// ---------------------------------------------------------------------------
// THIS SCRIPT TEMPORARILY EDITS A TRACKED SOURCE FILE, AND HAS TO.
//
// `MILKDROP_SOFTNESS` is a compile-time constant and deliberately **not** an
// authorable parameter — that is ADR-0124's decision, not an oversight, so there
// is no runtime lever to sweep. The only way to render this surface at four
// profiles is to build it four times. The constant is patched, built, rendered,
// and restored in a `finally`; the run ends by asserting the file is back to
// what it was and refuses to write an index if it is not.
//
// **A `finally` is not enough on its own and this has already bitten.** A run
// killed by a harness timeout during the fourth build left the constant at 0.00
// in the working tree, because a killed process runs no `finally`. So the
// restore is also wired to SIGINT/SIGTERM/SIGHUP and to `process.on("exit")`,
// all of which are synchronous writes of one small file.
//
// The whole sweep is four release builds of `rlx-core` plus its dependents, and
// it takes upward of ten minutes. Run it detached rather than under anything
// that will cut it off. If it is ever killed anyway, check that file first:
//   git diff core/src/render/scenes/warp_mesh/mod.rs
// ---------------------------------------------------------------------------
//
// **The subjects are `.milk` files, not hand-written bundles**, because Phase 8
// compares against the reference running *the same preset*. They are generated
// from `core/tests/fixtures/scratch-0108/wave-dots.milk` by flipping the wave
// flags, so both sides of the gate are driven by one file whose provenance is in
// the repo. The shipped `warp_mesh_milk.toml` is not usable here: its bundle
// drives the transform and draws no wave worth judging.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const PIN_SOURCE = "core/src/render/scenes/warp_mesh/mod.rs";
const PIN_RE = /pub const MILKDROP_SOFTNESS: f32 = [0-9.]+;/;
const BASE_MILK = "core/tests/fixtures/scratch-0108/wave-dots.milk";

/// Set `key=value` on a `.milk`, replacing the line if it is there.
function setKey(src, key, value) {
  const re = new RegExp(`^${key}=.*$`, "m");
  return re.test(src) ? src.replace(re, `${key}=${value}`) : `${src}\n${key}=${value}`;
}
const setAll = (src, pairs) =>
  Object.entries(pairs).reduce((s, [k, v]) => setKey(s, k, v), src);

/// A custom wave: one closed ring whose radius rides the sample value. The
/// built-in waveform is switched off (`fWaveAlpha=0`), so what the panel shows is
/// the **custom-wave path** rather than both at once — a different producer
/// reaching the same fragment (`draw.rs`'s `custom_waves`).
function customWave(src, thick) {
  const base = setAll(src, { fWaveAlpha: "0.000", bWaveDots: 0 });
  return [
    base,
    "wavecode_0_enabled=1",
    "wavecode_0_samples=512",
    "wavecode_0_busedots=0",
    `wavecode_0_bdrawthick=${thick ? 1 : 0}`,
    "wavecode_0_badditive=1",
    "wavecode_0_bspectrum=0",
    "wavecode_0_r=1.000",
    "wavecode_0_g=0.750",
    "wavecode_0_b=0.400",
    "wavecode_0_a=1.000",
    "wave_0_per_point1=t = sample * 6.2831853;",
    "wave_0_per_point2=rad = 0.34 + 0.10 * value1;",
    "wave_0_per_point3=x = 0.5 + rad * cos(t);",
    "wave_0_per_point4=y = 0.5 + rad * sin(t);",
    "",
  ].join("\n");
}

const SUBJECTS = [
  {
    name: "waveform-thin",
    note: "the built-in waveform at `THIN` — 0.0025 NDC-y, the width every MilkDrop waveform, motion vector and thin border is stroked at",
    edit: (s) => setAll(s, { bWaveDots: 0, bWaveThick: 0 }),
  },
  {
    name: "waveform-thick",
    note: "the same waveform at `THICK` — 0.006 NDC-y, which `draw.rs` records as MilkDrop's two-or-four-pass thick line reproduced as one stroke of twice the width",
    edit: (s) => setAll(s, { bWaveDots: 0, bWaveThick: 1 }),
  },
  {
    name: "customwave-thin",
    note: "a custom wave at `THIN` — a different producer (`custom_waves`) reaching the same fragment, with the built-in waveform switched off",
    edit: (s) => customWave(s, false),
  },
];

/// Both ends and two values between. `1.00` is the pin as it stands, and keeping
/// it is a legitimate verdict that closes the question rather than a null result.
const SOFTNESS = [1.0, 0.5, 0.25, 0.0];

/// 1080p and the small target. `THIN` is a **1.35 px** half-width at the first
/// and **1.0 px** at the second, where `fwidth` reaches the profile's cap
/// exactly — so this is where the two ends of the range differ least, and it has
/// to be looked at rather than assumed.
const SIZES = [
  { w: 1920, h: 1080, tag: "1080p" },
  { w: 1280, h: 800, tag: "1280x800" },
];

/// A sustained chord: the waveform has structure and it is the SAME structure at
/// the same hop on every panel, so the only thing differing across a row is the
/// profile. A `--set` stimulus would not do — it reaches the frame's scalars and
/// never the PCM, and this whole surface draws the PCM.
const SIGNAL = "chord";
const HOP = "60";

const slug = (softness) => `s${String(Math.round(softness * 100)).padStart(3, "0")}`;
const weight = (path) => `${Math.round(statSync(path).size / 1024)} KB`;
const digest = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

function run(bin, args) {
  execFileSync("cargo", ["run", "--release", "-q", "-p", bin, ...args], { stdio: "inherit" });
}
const shot = (args) => run("standalone", ["--example", "shot", "--", ...args]);

const outDir = resolve(process.argv[2] ?? "target/milk-softness");
mkdirSync(outDir, { recursive: true });

// --- the subjects, and the proof that they reach the fragment at all ---
const base = readFileSync(BASE_MILK, "utf8").replace(/\r\n/g, "\n");
for (const subject of SUBJECTS) {
  const milk = join(outDir, `${subject.name}.milk`);
  const toml = join(outDir, `${subject.name}.toml`);
  writeFileSync(milk, subject.edit(base));
  run("milkconv", ["--", milk, "--out", toml]);

  // **Non-vacuity, before a single panel is rendered.** `in_frame_geometry` is
  // the Plan 0069 diagnostic read off the line renderer's own draw, and it is
  // absent when nothing was stroked. Without this a subject whose wave failed to
  // convert renders four identical pictures of a warp field, and the gate reads
  // that as "the profile does nothing".
  const report = execFileSync(
    "cargo",
    ["run", "--release", "-q", "-p", "standalone", "--example", "shot", "--",
     "--preset-file", toml, "--report", "--json"],
    { encoding: "utf8" },
  );
  const geometry = /"in_frame_geometry":([0-9.]+)/.exec(report);
  if (!geometry) {
    throw new Error(
      `${subject.name} strokes nothing: the report carries no in_frame_geometry, ` +
      `so this subject would show a warp field and no line at all`,
    );
  }
  subject.geometry = Number(geometry[1]);
  console.log(`${subject.name}: in_frame_geometry ${subject.geometry}`);
}

// --- the sweep, one build per profile ---
const original = readFileSync(PIN_SOURCE, "utf8");
if (!PIN_RE.test(original)) throw new Error(`no MILKDROP_SOFTNESS in ${PIN_SOURCE}`);
const panels = new Map(); // `${subject}|${size}` -> [{softness, panel, hash}]

// Every way this process can end, including the ones that skip the `finally`
// below. `writeFileSync` is legal in an `exit` handler and in a signal handler;
// anything asynchronous would not run.
let patched = false;
const restore = () => {
  if (!patched) return;
  writeFileSync(PIN_SOURCE, original);
  patched = false;
  console.log(`restored ${PIN_SOURCE}`);
};
process.on("exit", restore);
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP", "SIGBREAK"]) {
  process.on(signal, () => {
    restore();
    process.exit(130);
  });
}

try {
  for (const softness of SOFTNESS) {
    console.log(`\n=== building with MILKDROP_SOFTNESS = ${softness} ===`);
    patched = true;
    writeFileSync(
      PIN_SOURCE,
      original.replace(PIN_RE, `pub const MILKDROP_SOFTNESS: f32 = ${softness.toFixed(2)};`),
    );
    execFileSync(
      "cargo",
      ["build", "--release", "-q", "-p", "standalone", "--example", "shot"],
      { stdio: "inherit" },
    );
    for (const subject of SUBJECTS) {
      for (const size of SIZES) {
        const panel = join(outDir, `${subject.name}-${size.tag}-${slug(softness)}.png`);
        shot([
          "--preset-file", join(outDir, `${subject.name}.toml`),
          "--signal", SIGNAL,
          "--frame-at", HOP,
          "--size", `${size.w}x${size.h}`,
          "--out", panel,
        ]);
        const key = `${subject.name}|${size.tag}`;
        if (!panels.has(key)) panels.set(key, []);
        panels.get(key).push({ softness, panel, hash: digest(panel) });
      }
    }
  }
} finally {
  restore();
}

if (readFileSync(PIN_SOURCE, "utf8") !== original) {
  throw new Error(`${PIN_SOURCE} did not come back to what it was — fix it before anything else`);
}

// --- the index, including the rig ---
const index = [
  "# `warp_mesh` stroke against the reference (Plan 0114 Phase 7)",
  "",
  "The artifacts Phase 8 judges. Every panel in a row is the same `.milk`, the same",
  "synthetic audio and the same hop — only `MILKDROP_SOFTNESS` differs. **`1.00` is",
  "the pin as it stands**, the profile the whole MilkDrop conversion was judged",
  "under, and keeping it is a verdict that closes the question rather than a null",
  "result.",
  "",
  "Phase 8 owes a number, and one other answer: **is MilkDrop's line in fact",
  "*harder* than this engine's falloff?** If the profile turns out not to be what",
  "differs, that is also a result, and it routes to ADR-0113's fidelity ledger",
  "rather than back into this plan.",
  "",
  `Signal \`${SIGNAL}\`, frame at hop ${HOP}. Generated by \`node scripts/milk-softness.mjs\`.`,
  "",
  "## Putting the reference beside it",
  "",
  "- **Reference side:** `foo_vis_milk2` in foobar2000 v2. It reads presets **only**",
  "  from `%APPDATA%\\foobar2000-v2\\milkdrop2\\` — there is no preferences setting.",
  "  Copy the `.milk` files from this directory into it. In the MilkDrop window,",
  "  `L` opens the preset browser and `SCROLL LOCK` pins the current preset.",
  "- **Our side:** `target\\release\\ritmolux.exe`. It does not read `.milk`, so point",
  "  `RLX_PRESET_DIR` at this directory — the `.toml` bundles beside them are the",
  "  same presets converted, and `Tab` filters by name.",
  "- **One track in foobar feeds both**: the component reads it directly and `ritmolux`",
  "  picks it up over loopback. Play the same thing for both sides.",
  "- **The panels below are the spread; the app is the check.** `ritmolux` builds at",
  "  whatever `MILKDROP_SOFTNESS` is in the tree (`1.00` unless you change it), so",
  "  the live side shows one profile. Pick a candidate off the panels, then set the",
  "  constant to it and rebuild before the side-by-side.",
  "",
  "## Subjects",
  "",
  "| subject | `in_frame_geometry` | what it is |",
  "|---|---|---|",
  ...SUBJECTS.map((s) => `| \`${s.name}\` | ${s.geometry.toFixed(4)} | ${s.note} |`),
  "",
  "`in_frame_geometry` is the line renderer's own draw diagnostic, read before any",
  "panel was rendered: it is absent when nothing was stroked, so a number here is",
  "the proof that these are pictures of a **stroke** and not of a warp field.",
  "",
];

for (const subject of SUBJECTS) {
  index.push(`## ${subject.name}`, "");
  for (const size of SIZES) {
    const row = panels.get(`${subject.name}|${size.tag}`) ?? [];
    const twin = row.map((p) => row.find((q) => q.hash === p.hash) ?? p);
    const collapsed = new Set(row.map((p) => p.hash)).size < row.length;
    index.push(
      `### ${size.w}x${size.h}`,
      "",
      "| `softness` | panel | weight | reads as |",
      "|---|---|---|---|",
      ...row.map((p, i) => {
        const name = p.panel.split(/[\\/]/).pop();
        const reads =
          twin[i].softness === p.softness
            ? "distinct"
            : `**identical to \`${twin[i].softness.toFixed(2)}\`**`;
        const pin = p.softness === 1 ? " (the pin today)" : "";
        return `| \`${p.softness.toFixed(2)}\`${pin} | [\`${name}\`](${name}) | ${weight(p.panel)} | ${reads} |`;
      }),
      "",
    );
    if (collapsed) {
      index.push(
        "> **The range collapses here.** The edge is floored at one pixel of the",
        "> render target, and MilkDrop's own widths sit at 1.0–1.35 px of half-width",
        "> — right at that floor. Where two panels are identical the profile has no",
        "> room to differ, which is ADR-0124's stated limit and is itself an answer:",
        "> on this subject at this size, the pin's value cannot matter.",
        "",
      );
    }
  }
}

writeFileSync(join(outDir, "index.md"), index.join("\n"));
console.log(`\npanels, subjects + index in ${outDir}`);
