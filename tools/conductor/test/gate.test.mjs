// The conductor's gate: what it runs by default, and how a step whose tool is absent skips.

import assert from "node:assert/strict";
import { test } from "node:test";

import { defaultGate, runGate } from "../lib/gate.mjs";
import { tmp } from "./helpers.mjs";

test("the default gate carries the pre-push suites it once missed", () => {
  const byName = Object.fromEntries(defaultGate().map((c) => [c.name, c]));
  assert.deepEqual(byName["conductor tests"].cmd, ["node", "--test", "tools/conductor/test/*.test.mjs"]);
  assert.deepEqual(byName["sd-filter tests"].cmd, ["python3", "tools/sd-filter/test_sd_filter.py"]);
  assert.deepEqual(byName["sd-filter tests"].onlyIfCommand, ["python3", "--version"]);
  // The suites run before the cargo steps, so a red there costs no Rust build.
  const names = defaultGate().map((c) => c.name);
  assert.ok(names.indexOf("sd-filter tests") < names.indexOf("cargo fmt"));
});

test("a step whose onlyIfCommand does not run is skipped, and one whose command runs is not", async () => {
  const node = process.execPath;
  const g = await runGate({
    cwd: tmp(),
    logDir: tmp(),
    label: "skip",
    commands: [
      { name: "absent tool", cmd: [node, "-e", "process.exit(1)"], onlyIfCommand: ["rlx-no-such-command-anywhere", "--version"] },
      { name: "present tool", cmd: [node, "-e", "process.exit(0)"], onlyIfCommand: [node, "--version"] },
    ],
  });
  assert.equal(g.ok, true);
  assert.deepEqual(g.ran, ["present tool"]);
});
