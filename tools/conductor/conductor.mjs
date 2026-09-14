#!/usr/bin/env node
// The conductor (ADR-0205): takes approved plans off tools/conductor/queue.json and runs them in
// worktree lanes, one fresh headless `claude -p` session per same-owner run of phases, a conductor
// gate, a fresh review-and-close session, a fast-forward of main and the lane's removal. It never
// pushes. Every judgement it cannot make parks the plan.
//
//   node tools/conductor/conductor.mjs check      preflight only: local.json, CLI version, queue
//
// Runs from the main checkout. Everything it writes at runtime lives under tools/conductor/state/.

import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { loadLocal, loadQueue } from "./lib/queue.mjs";
import { loadState } from "./lib/state.mjs";

export const TOOL_DIR = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(TOOL_DIR, "..", "..");

// The CLI versions tools/conductor/spike/README.md's evidence table was produced on. A version not
// listed here is refused; re-running the spike probe is how one is added.
export const VERIFIED_CLI = ["2.1.270"];

export function paths({ repo = REPO, toolDir = TOOL_DIR } = {}) {
  return {
    repo,
    toolDir,
    queue: join(toolDir, "queue.json"),
    local: join(toolDir, "local.json"),
    stateDir: join(toolDir, "state"),
    digest: join(toolDir, "digest.md"),
    settings: join(toolDir, "settings.conductor.json"),
    prompts: join(toolDir, "prompts"),
    withLock: join(toolDir, "with-lock.mjs"),
  };
}

export function claudeVersion(claude) {
  const [bin, ...pre] = claude;
  const r = spawnSync(bin, [...pre, "--version"], { encoding: "utf8" });
  if (r.status !== 0) return { error: `could not run ${claude.join(" ")} --version: ${r.error?.message ?? r.stderr}` };
  const m = (r.stdout ?? "").match(/(\d+\.\d+\.\d+)/);
  return m ? { version: m[1], raw: r.stdout.trim() } : { error: `unrecognised --version output: ${r.stdout}` };
}

/**
 * Everything that must hold before a run starts. Returns { errors, local, queue, state, claude }.
 * `claude` overrides local.json's command vector (tests pass the fake).
 */
export function preflight(p = paths(), { claude } = {}) {
  const errors = [];
  const { errors: localErrors, local } = loadLocal(p.local);
  errors.push(...localErrors);
  const command = claude ?? local?.claude ?? ["claude"];
  const v = claudeVersion(command);
  if (v.error) errors.push(v.error);
  else if (!VERIFIED_CLI.includes(v.version)) {
    errors.push(
      `claude ${v.version} is not a verified CLI version (verified: ${VERIFIED_CLI.join(", ")}); ` +
        `re-run tools/conductor/spike/probe.mjs and record the evidence before adding it`,
    );
  }
  const state = loadState(p.stateDir);
  const queue = loadQueue(p.queue, p.repo, new Set(Object.keys(state.plans)));
  errors.push(...queue.errors);
  return { errors, local, queue, state, claude: command };
}

async function main(argv) {
  const [command] = argv;
  if (command === "check") {
    const r = preflight();
    if (r.errors.length) {
      for (const e of r.errors) console.error(`conductor: ${e}`);
      return 1;
    }
    console.log("conductor: preflight OK");
    return 0;
  }
  console.error("usage: node tools/conductor/conductor.mjs check");
  return 2;
}

const norm = (p) => (process.platform === "win32" ? resolve(p).toLowerCase() : resolve(p));
if (process.argv[1] && norm(fileURLToPath(import.meta.url)) === norm(process.argv[1])) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
