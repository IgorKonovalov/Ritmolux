#!/usr/bin/env node
// Is this worktree's tree green in the suite ledger? The pre-push hook's question (ADR-0237).
//
//   node tools/conductor/suite-record.mjs [dir]
//
// Exit 0 and one line naming the record — its tree, who ran it, when, and nextest's summary — when
// `dir` (default: the current directory) is clean and the ledger's newest full-suite run for its
// tree exited 0. Exit 1 and one line saying why not otherwise. The hook skips its test step on exit 0
// only, so a crash here reads as "run the suite".
//
// One reader, one answer: the ledger's location is `suiteLedger` in `with-lock.mjs` and the lookup is
// `greenRecord` in `lib/ledger.mjs`, the same pair the wrapper consults before it skips a full suite,
// so the hook cannot disagree with the conductor about which trees are proved. `greenRecord` reads
// only exact `cargo nextest run --workspace` runs: a `-P fast` line — run by hand, or served under
// ADR-0211 — never answers yes here. Nothing is written; the hook is a reader of the ledger only.

import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { cleanTree, greenRecord, treeOf } from "./lib/ledger.mjs";
import { suiteLedger } from "./with-lock.mjs";

/** `{ served, line }` for the worktree at `cwd`. */
export function lookup(cwd, env = process.env) {
  const ledger = suiteLedger(cwd, env);
  if (!ledger) return { served: false, line: "no suite ledger belongs to this checkout" };
  const tree = cleanTree(cwd);
  if (!tree) {
    const at = treeOf(cwd);
    return { served: false, line: `the worktree is dirty, so tree ${at ? at.slice(0, 7) : "?"} names nothing that was tested` };
  }
  const record = greenRecord(ledger.path, tree);
  if (!record) return { served: false, line: `tree ${tree.slice(0, 7)} has no green full-suite record in ${ledger.path}` };
  return {
    served: true,
    line: `tree ${tree.slice(0, 7)} is green in the suite ledger, run by ${record.by} at ${record.at}: ${record.summary ?? "no summary"}`,
  };
}

const norm = (p) => (process.platform === "win32" ? resolve(p).toLowerCase() : resolve(p));
if (process.argv[1] && norm(fileURLToPath(import.meta.url)) === norm(process.argv[1])) {
  const r = lookup(resolve(process.argv[2] ?? "."));
  console.log(r.line);
  process.exit(r.served ? 0 : 1);
}
