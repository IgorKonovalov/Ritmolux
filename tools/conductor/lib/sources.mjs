// Whether a running conductor is still executing the code on disk.
//
// `run` is one long-lived Node process that imports its modules once, so a change to them merged
// while it is up does not reach it: it keeps deciding with the logic it started with. At start the run
// records a content hash of its module set in state/conductor.sources.json beside its pid; the run
// compares it against the files on each look and pauses when they differ, and `status`, `resume` and
// `park` compare it too and say so, since the live run is what acts on their asks.
//
// Content, never a timestamp: a checkout that rewrites a file with identical bytes is not a change.
// Nothing here restarts the run or stops a session: an in-flight session killed mid-plan loses its
// work, which is the harm ADR-0205 refuses for backgrounded commands.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

import { writeAtomic } from "./state.mjs";

/** The entry points beside lib/ that `run` loads; every `lib/*.mjs` is loaded as well. */
const TOP_LEVEL = ["conductor.mjs", "with-lock.mjs"];

/** The module set under `toolDir`, as forward-slashed relative paths, sorted. */
export function sourceFiles(toolDir) {
  const lib = join(toolDir, "lib");
  const libFiles = existsSync(lib) ? readdirSync(lib).filter((f) => f.endsWith(".mjs")).map((f) => `lib/${f}`) : [];
  return [...TOP_LEVEL.filter((f) => existsSync(join(toolDir, f))), ...libFiles].sort();
}

const sha = (data) => createHash("sha256").update(data).digest("hex");

/** `{ hash, files: { rel: sha256 } }` over the module set as it is on disk now. */
export function sourceDigest(toolDir) {
  const files = {};
  for (const rel of sourceFiles(toolDir)) {
    try {
      files[rel] = sha(readFileSync(join(toolDir, rel)));
    } catch {
      files[rel] = "unreadable";
    }
  }
  return { hash: sha(JSON.stringify(files)), files };
}

/** Every path added, removed or changed between two digests, sorted. */
export function changedSources(before, after) {
  const names = new Set([...Object.keys(before.files), ...Object.keys(after.files)]);
  return [...names].filter((rel) => before.files[rel] !== after.files[rel]).sort();
}

const sourcesFile = (stateDir) => join(stateDir, "conductor.sources.json");

/** Records what the run with `pid` loaded. */
export function recordSources(stateDir, pid, digest) {
  writeAtomic(sourcesFile(stateDir), JSON.stringify({ pid, ...digest }, null, 2) + "\n");
}

export function clearSources(stateDir) {
  rmSync(sourcesFile(stateDir), { force: true });
}

function recordedSources(stateDir) {
  try {
    return JSON.parse(readFileSync(sourcesFile(stateDir), "utf8"));
  } catch {
    return null;
  }
}

/**
 * The files that changed under the live run `pid` since it recorded its module set, or null when
 * none did, or when there is no record for that pid to compare against.
 */
export function staleSince(stateDir, toolDir, pid) {
  const loaded = recordedSources(stateDir);
  if (!loaded || loaded.pid !== pid || typeof loaded.files !== "object") return null;
  const now = sourceDigest(toolDir);
  return now.hash === loaded.hash ? null : changedSources(loaded, now);
}

/** The one line naming a stale run: which process, and which files moved under it. */
export function staleLine(pid, changed) {
  return (
    `the running conductor (pid ${pid}) loaded tools/conductor/ sources that have changed on disk since it started ` +
    `(${changed.join(", ")}); it decides with the old code, and pauses once its plans in flight finish - start \`run\` again after it ends`
  );
}
