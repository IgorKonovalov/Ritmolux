#!/usr/bin/env node
// Render one filmstrip per candidate tuple path — the evidence ADR-0093 gates
// the morph paths on.
//
// Rationale: Plan 0079 Phase 5. A path between two roster entries ships only
// where a rendered end-to-end sweep shows the figure staying recognisable along
// the walk, and Phase 6 is the human gate that judges these. The IFS five-pair
// sweep (ADR-0075) is the precedent and the shape.
//
// Usage:  node scripts/tuple-paths.mjs [out-dir]
// Writes  <out-dir>/<family>-<from>-<to>.png  and  <out-dir>/index.md
// Default out-dir: target/tuple-paths
//
// Each strip is one preset per `morph` step, tiled by the `shot` example's
// `--all` contact sheet — the same tooling the candidate sheets use. A cell is
// labeled with its morph position, so the strip reads left to right as the walk.
//
// WHICH PAIRS. Not all of them: the rosters are 12 and 13 entries, so the
// complete set is 66 to 78 pairs per family and most of those are two unrelated
// figures with nothing between them. The pairs below are the ones the roster
// makes *plausible* — neighbours along the one-dimensional sweeps (Thomas's `a`,
// Lorenz's `rho`), plus a few structurally-related figures on the two discrete
// maps, plus the canonical-to-notable pair each family obviously wants. That
// selection is a judgement, and a curator who wants a pair that is not here can
// add it to the table.

import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const FAMILY_SRC = "core/src/render/scenes/particles/family.rs";
/// Steps along each walk, endpoints included.
const STEPS = 7;

/// Marks a pair the engine refuses to build a walk for, because some figure
/// along it cannot be framed — a De Jong or Clifford tuple partway between two
/// others can collapse to a fixed point, whose extent is zero and which
/// therefore has no scale to render at.
///
/// **These are not rendered**, and that is the point: a preset naming such a
/// pair sits on its near end, so the strip would be seven identical cells and
/// would read as "the walk does nothing" rather than "there is no walk". The
/// source of truth is `the_swept_pairs_report_whether_they_have_a_walk` in
/// `core/src/render/scenes/particles/tests.rs`, which prints the list; this
/// table mirrors its verdict so the index can say so out loud.
const NO_WALK = "no-walk";

const PAIRS = {
  de_jong: [
    [0, 6, "the canonical ribbon to the four-lobed bow-tie", NO_WALK],
    [0, 10, "ribbon to the dense orb"],
    [6, 2, "bow-tie to the vaulted arcs"],
    [9, 12, "the bare S-curve to the fountain"],
    [1, 3, "shell to kidney - two figures that already rhyme"],
  ],
  clifford: [
    [0, 9, "the canonical to the woven disc", NO_WALK],
    [0, 5, "canonical to the oblique ring"],
    [9, 10, "one disc to the other - the closest pair in the roster", NO_WALK],
    [7, 12, "three crescents to the chevron"],
    [2, 3, "two nautilus figures", NO_WALK],
  ],
  thomas: [
    [1, 12, "the whole band, a = 0.03 to 0.22"],
    [2, 6, "0.05 to 0.13, inside the chaos"],
    [5, 8, "0.11 to 0.17, pinwheel to figure-eight"],
    [8, 11, "0.17 to 0.208, up to the edge of chaos"],
    [0, 9, "0.19 to 0.20 - the shortest step in the roster"],
  ],
  lorenz: [
    [0, 1, "the butterfly to the torus knot"],
    [0, 4, "rho 28 to 35"],
    [4, 5, "rho 35 to 60"],
    [2, 1, "rho 92 to the knot at 100"],
    [0, 11, "beta 2.667 to 4.0, rho held"],
  ],
};

/// Read a family's roster length out of the Rust source, so a pair naming an
/// entry that no longer exists fails here rather than rendering a clamped
/// duplicate.
function rosterLen(src, family) {
  const camel = { de_jong: "DeJong", clifford: "Clifford", thomas: "Thomas", lorenz: "Lorenz" }[family];
  const fn = src.slice(src.indexOf("fn extra_tuples("));
  const arm = fn.indexOf(`AttractorFamily::${camel} =>`);
  const open = fn.indexOf("[", arm);
  let depth = 0;
  let close = open;
  for (; close < fn.length; close += 1) {
    if (fn[close] === "[") depth += 1;
    if (fn[close] === "]") {
      depth -= 1;
      if (depth === 0) break;
    }
  }
  const body = fn.slice(open, close + 1);
  return [...body.matchAll(/\[\s*-?[\d.]+(?:\s*,\s*-?[\d.]+)*\s*,?\s*\]/g)].length;
}

// A bare preset per cell. `spin = 0` deliberately: a walk has to be judged on
// the figure changing, and a figure that is also turning hides which of the two
// moved. `reseed` is off for the same reason.
//
// **EACH CELL WALKS TO ITS POSITION RATHER THAN STARTING AT IT**, and the first
// draft of this script got that wrong. A preset pinned at `morph = 1` seeds its
// cloud from the NEAR end's attractor and then jumps the coefficients to the far
// end, so what renders is the transient of a figure falling onto a new
// attractor - which on the Lorenz pairs takes seconds and looks nothing like the
// entry the path names. That is an artifact of the jump, not a property of the
// walk: a real preset eases `morph`, and the cloud tracks the figure as it
// moves. So `morph` ramps here at 0.2/s and holds at the cell's position, which
// is what a slow bound walk actually does.
const PRESET = (name, family, from, to, morph) => `system = "attractor"
name = "${name}"

[particles]
family = "${family}"
density = 0.02
tuple_from = ${from}
tuple_to = ${to}

[params]
morph = "min(time * 0.2, ${morph})"
brightness = "1.5"
fade = "0.82"
size = "0.22"
spin = "0"
perspective = "0.18"
`;

const outDir = resolve(process.argv[2] ?? "target/tuple-paths");
mkdirSync(outDir, { recursive: true });
const src = readFileSync(FAMILY_SRC, "utf8");

const index = [
  "# Attractor tuple path sweeps (Plan 0079 Phase 5)",
  "",
  "One filmstrip per candidate path. Each reads left to right as `morph` walks",
  "0 to 1; the two ends are the roster entries the path names, and everything",
  "between is a figure measured at load rather than interpolated.",
  "",
  "**What Phase 6 is judging:** does the figure stay recognisable across the",
  "walk, or does it pass through mush in the middle? A pair that mushes does not",
  "ship, and zero surviving pairs is a legitimate recorded outcome.",
  "",
];

for (const [family, pairs] of Object.entries(PAIRS)) {
  const len = rosterLen(src, family) + 1;
  index.push(`## ${family}`, "");
  for (const [from, to, why, flag] of pairs) {
    if (from >= len || to >= len) {
      throw new Error(`${family} pair ${from}->${to} is past its ${len}-entry roster`);
    }
    if (flag === NO_WALK) {
      console.log(`skipping ${family} ${from} -> ${to}: no walk exists`);
      index.push(
        `### ${from} -> ${to} — ${why}`,
        "",
        "**No walk.** Some figure along this pair cannot be framed — a tuple",
        "partway between two others can collapse to a fixed point, whose extent",
        "is zero. A preset naming this pair sits on its near end and `morph` does",
        "nothing, so there is no strip to judge.",
        "",
        "Verdict: refused by measurement, not by eye.",
        "",
      );
      continue;
    }
    const strip = join(outDir, `${family}-${from}-${to}.png`);
    if (existsSync(strip)) {
      console.log(`keeping ${family} ${from} -> ${to} (already rendered)`);
      index.push(
        `### ${from} -> ${to} — ${why}`,
        "",
        `![${family} ${from} to ${to}](${family}-${from}-${to}.png)`,
        "",
        "Verdict: _(Phase 6)_",
        "",
      );
      continue;
    }
    const dir = mkdtempSync(join(tmpdir(), `lmv-path-${family}-`));
    try {
      for (let i = 0; i < STEPS; i += 1) {
        const morph = (i / (STEPS - 1)).toFixed(3);
        writeFileSync(
          join(dir, `${String(i).padStart(2, "0")}.toml`),
          PRESET(`MORPH ${morph}`, family, from, to, morph),
        );
      }
      console.log(`sweeping ${family} ${from} -> ${to}: ${why}`);
      execFileSync(
        "cargo",
        [
          "run", "--release", "-q", "-p", "standalone", "--example", "shot", "--",
          "--presets", dir,
          "--all",
          "--out", strip,
          "--size", "420x420",
          "--frames", "480",
        ],
        { stdio: "inherit" },
      );
      index.push(
        `### ${from} -> ${to} — ${why}`,
        "",
        `![${family} ${from} to ${to}](${family}-${from}-${to}.png)`,
        "",
        "Verdict: _(Phase 6)_",
        "",
      );
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
}

writeFileSync(join(outDir, "index.md"), index.join("\n"));
console.log(`\n${Object.values(PAIRS).flat().length} strips + index in ${outDir}`);
