#!/usr/bin/env node
// A stand-in for `gh`, answering the two calls check-upstream-ci.mjs makes from the scenario file
// RLX_FAKE_GH names:
//
//   { "exit": N, "stderr": "..." }             every call fails that way (unauthenticated, offline)
//   { "runs": [...], "jobs": { "<id>": [...] } }
//
// `run list` honours `--workflow` (matched against each run's `workflowFile`), `--branch`, `--status`
// and `--limit`, so a scenario can hold runs of other workflows and prove the reader never sees them.
// `run view <id> --json jobs` prints that run's jobs.

import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const scenario = JSON.parse(readFileSync(process.env.RLX_FAKE_GH, "utf8"));
const flag = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};

if (typeof scenario.exit === "number") {
  process.stderr.write(`${scenario.stderr ?? ""}\n`);
  process.exit(scenario.exit);
}

if (args[0] === "run" && args[1] === "list") {
  const workflow = flag("--workflow");
  const branch = flag("--branch");
  const status = flag("--status");
  const limit = Number(flag("--limit") ?? 20);
  const runs = (scenario.runs ?? [])
    .filter((r) => !workflow || r.workflowFile === workflow)
    .filter((r) => !branch || r.headBranch === branch)
    .filter((r) => !status || r.status === status)
    .slice(0, limit)
    .map(({ databaseId, conclusion, headSha, url, workflowName }) => ({ databaseId, conclusion, headSha, url, workflowName }));
  process.stdout.write(JSON.stringify(runs));
  process.exit(0);
}

if (args[0] === "run" && args[1] === "view") {
  process.stdout.write(JSON.stringify({ jobs: scenario.jobs?.[args[2]] ?? [] }));
  process.exit(0);
}

process.stderr.write(`fake gh: unexpected call: ${args.join(" ")}\n`);
process.exit(2);
