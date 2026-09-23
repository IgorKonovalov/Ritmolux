// The test suite leaves nothing behind in the OS temp directory: tmp() removes its directories when
// the process exits, the next process sweeps what a killed run left, and no test file makes a temp
// directory any other way.

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

import { TEST_DIR } from "./helpers.mjs";

const HELPERS = pathToFileURL(join(TEST_DIR, "helpers.mjs")).href;
const MAKE_DIRS =
  `import { tmp } from ${JSON.stringify(HELPERS)};` +
  `import { writeFileSync } from "node:fs";` +
  `const a = tmp(); const b = tmp("rlx-probe-");` +
  `writeFileSync(a + "/f.txt", "x");` +
  `console.log(JSON.stringify([a, b]));`;

function childEnv() {
  const env = { ...process.env };
  delete env.RLX_KEEP_TMP;
  return env;
}

test("a process's temp directories are gone once it exits", () => {
  const r = spawnSync(process.execPath, ["--input-type=module", "-e", MAKE_DIRS], { encoding: "utf8", env: childEnv() });
  assert.equal(r.status, 0, r.stderr);
  const dirs = JSON.parse(r.stdout);
  assert.equal(dirs.length, 2);
  for (const d of dirs) assert.equal(existsSync(d), false, `${d} survived its process`);
});

test("the next process sweeps the directories of a run that was killed", async () => {
  const hang = MAKE_DIRS + `setInterval(() => {}, 1000);`;
  const child = spawn(process.execPath, ["--input-type=module", "-e", hang], { stdio: ["ignore", "pipe", "inherit"], env: childEnv() });
  const dirs = await new Promise((done) => {
    let out = "";
    child.stdout.on("data", (d) => {
      out += d;
      if (out.includes("\n")) done(JSON.parse(out));
    });
  });
  for (const d of dirs) assert.equal(existsSync(d), true);
  const exited = new Promise((done) => child.on("exit", done));
  child.kill("SIGKILL");
  await exited;
  for (const d of dirs) assert.equal(existsSync(d), true, "a killed process runs no exit hook");

  const r = spawnSync(process.execPath, ["--input-type=module", "-e", `import ${JSON.stringify(HELPERS)};`], { encoding: "utf8", env: childEnv() });
  assert.equal(r.status, 0, r.stderr);
  for (const d of dirs) assert.equal(existsSync(d), false, `${d} outlived the sweep`);
});

test("no test file makes a temp directory except through tmp()", () => {
  const offenders = [];
  for (const name of readdirSync(TEST_DIR)) {
    if (!name.endsWith(".mjs") || name === "helpers.mjs" || name === "tmp.test.mjs") continue;
    readFileSync(join(TEST_DIR, name), "utf8")
      .split("\n")
      .forEach((line, i) => {
        if (/\bmkdtemp(Sync)?\s*\(|\btmpdir\s*\(/.test(line)) offenders.push(`${name}:${i + 1}: ${line.trim()}`);
      });
  }
  assert.deepEqual(offenders, [], "use tmp() from helpers.mjs, which removes what it makes");
});
