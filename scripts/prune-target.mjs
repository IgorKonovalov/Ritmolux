#!/usr/bin/env node
// Delete what cargo no longer reports from the target directory's dev `deps/`.
// A MAINTENANCE TOOL, neither a gate nor a renderer: nothing runs it but a
// person whose disk is filling, and it never decides whether a build is right.
//
// `target/debug/deps/` keeps every generation of every unit it has ever built. A
// dependency bump, a feature flip or an edit that changes a unit's metadata hash
// leaves the old `.rlib`/`.rmeta`/`.exe`/`.pdb` beside the new one, and a pinned
// stable toolchain has no `cargo clean --gc` to collect them (backlog 0184).
//
// Usage:
//   node scripts/prune-target.mjs                 dry run: list what would go, and the bytes
//   node scripts/prune-target.mjs --apply         delete it
//   node scripts/prune-target.mjs --verify-fresh  run the loop and exit 1 unless every
//                                                 artifact cargo reports is fresh
//
// Exit 0 = done (or, with --verify-fresh, everything was fresh). Exit 1 = a loop
// command failed, a reported artifact could not be matched to a file in `deps/`,
// or --verify-fresh found a unit cargo had to rebuild.
//
// THE LIVE SET IS WHAT CARGO ITSELF REPORTS, not an age rule. The everyday loop's
// three commands run with JSON messages:
//
//   cargo build --workspace --message-format=json
//   cargo clippy --workspace --all-targets --message-format=json -- -D warnings
//   cargo nextest run --workspace -P fast --no-run --cargo-message-format json
//
// and every `filenames` entry of every `compiler-artifact` message, fresh or not,
// is live. Cargo reports a unit it did not rebuild too, so a warm run costs
// seconds and still names everything. An mtime rule would be wrong: a fresh unit
// is not rewritten, so its mtime is as old as the stale generation beside it.
//
// A `deps/` file is kept when its `-<16 hex>` metadata hash is the hash of a
// live file, so a live `.exe` keeps its `.pdb` and `.d`, and a live `.rlib`
// keeps its `.rmeta`. An artifact cargo reports OUTSIDE `deps/` (a binary copied
// up to `target/debug/`) is matched to the `deps/` file it was linked from by
// file identity (a hard link), else by name, size and content. One that matches
// nothing is an error rather than a guess, and nothing is deleted. A `deps/`
// file with no metadata hash in its name is never touched.
//
// WHAT IT NEEDS:
//   - NO OTHER CARGO PROCESS RUNNING IN THE SAME CHECKOUT. A concurrent build can
//     write a unit this run never saw reported, and --apply would delete it.
//   - The loop to be green. A unit that fails to compile is never reported, so its
//     last good output would read as dead; a failing command stops the script.
//   - cargo-nextest, as the everyday loop does.
//
// WHAT IT COSTS WHEN WRONG: a rebuild, never correctness. Cargo's fingerprint sees
// a missing output as dirty and rebuilds the unit. --verify-fresh straight after
// --apply is how to see that nothing the loop needs was taken.
//
// THE TARGET DIRECTORY is `cargo metadata`'s `target_directory`, so a
// `CARGO_TARGET_DIR` or `build.target-dir` redirect is followed; no path here is
// built from the source tree. Only the host `debug` profile is read: a
// `--target <triple>` or `--release` build keeps its own `deps/`, untouched.
//
// `incremental/` is not pruned here; docs/developing.md's "Disk" section says what
// bounds it.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const args = new Set(process.argv.slice(2));
const apply = args.has('--apply');
const verifyFresh = args.has('--verify-fresh');
for (const arg of args) {
  if (!['--apply', '--verify-fresh'].includes(arg)) {
    console.error(`prune-target: unknown argument ${arg}`);
    process.exit(1);
  }
}
if (apply && verifyFresh) {
  console.error('prune-target: --apply and --verify-fresh are separate runs; run --verify-fresh after --apply');
  process.exit(1);
}

const LOOP = [
  ['cargo', ['build', '--workspace', '--message-format=json']],
  ['cargo', ['clippy', '--workspace', '--all-targets', '--message-format=json', '--', '-D', 'warnings']],
  ['cargo', ['nextest', 'run', '--workspace', '-P', 'fast', '--no-run', '--cargo-message-format', 'json']],
];

// Windows paths compare case-insensitively; a drive letter's case varies by caller.
const norm = (p) => (process.platform === 'win32' ? path.resolve(p).toLowerCase() : path.resolve(p));
const HASHED = /^(.*)-([0-9a-f]{16})(\..+)?$/;

function run(cmd, cmdArgs, { json }) {
  const result = spawnSync(cmd, cmdArgs, {
    stdio: ['ignore', json ? 'pipe' : 'inherit', 'inherit'],
    encoding: 'utf8',
    maxBuffer: 1 << 30,
  });
  if (result.error || result.status !== 0) {
    console.error(`prune-target: \`${cmd} ${cmdArgs.join(' ')}\` failed (${result.error ?? `exit ${result.status}`})`);
    process.exit(1);
  }
  return result.stdout ?? '';
}

function targetDirectory() {
  const metadata = JSON.parse(run('cargo', ['metadata', '--format-version', '1', '--no-deps'], { json: true }));
  return metadata.target_directory;
}

/** Every `compiler-artifact` message the loop's three commands print. */
function artifacts() {
  const out = [];
  for (const [cmd, cmdArgs] of LOOP) {
    console.error(`prune-target: ${cmd} ${cmdArgs.join(' ')}`);
    for (const line of run(cmd, cmdArgs, { json: true }).split('\n')) {
      if (!line.startsWith('{')) continue;
      let message;
      try {
        message = JSON.parse(line);
      } catch {
        continue;
      }
      if (message.reason === 'compiler-artifact') out.push(message);
    }
  }
  return out;
}

const identity = (file) => {
  const s = fs.statSync(file, { bigint: true });
  return `${s.dev}:${s.ino}`;
};

function sameContent(a, b) {
  const sa = fs.statSync(a);
  const sb = fs.statSync(b);
  return sa.size === sb.size && fs.readFileSync(a).equals(fs.readFileSync(b));
}

const messages = artifacts();
if (messages.length === 0) {
  console.error('prune-target: the loop reported no compiler-artifact message; refusing to treat everything as dead');
  process.exit(1);
}

if (verifyFresh) {
  const stale = messages.filter((m) => m.fresh !== true);
  for (const m of stale) console.log(`not fresh: ${m.target?.name} (${m.package_id})`);
  console.log(`prune-target: ${messages.length - stale.length} of ${messages.length} artifacts fresh`);
  process.exit(stale.length === 0 ? 0 : 1);
}

const target = targetDirectory();
const deps = path.join(target, 'debug', 'deps');
if (!fs.existsSync(deps)) {
  console.log(`prune-target: ${deps} does not exist; nothing to prune`);
  process.exit(0);
}
const depsNorm = norm(deps);
const depsFiles = fs.readdirSync(deps).filter((f) => fs.statSync(path.join(deps, f)).isFile());

const liveHashes = new Set();
const outside = [];
for (const m of messages) {
  for (const file of m.filenames ?? []) {
    if (norm(path.dirname(file)) === depsNorm) {
      const hashed = HASHED.exec(path.basename(file));
      if (hashed) liveHashes.add(hashed[2]);
    } else if (fs.existsSync(file)) {
      outside.push(file);
    }
  }
}

// An artifact reported outside deps/ is a copy of a deps/ file; find which.
const unmatched = [];
for (const file of outside) {
  const stem = path.basename(file, path.extname(file));
  const ext = path.extname(file);
  const hashedSibling = new RegExp(`^${stem.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}-([0-9a-f]{16})${ext.replace('.', '\\.')}$`);
  const self = identity(file);
  // A binary on MSVC is written to deps/ under its own unhashed name (so its
  // `.pdb` keeps that name), and an unhashed file is never a candidate below.
  const unhashed = path.join(deps, path.basename(file));
  if (fs.existsSync(unhashed) && (identity(unhashed) === self || sameContent(unhashed, file))) continue;
  const candidates = depsFiles.filter((f) => hashedSibling.test(f));
  if (candidates.length === 0) continue; // not linked out of deps/ (an example, a build script)
  const linked =
    candidates.find((f) => identity(path.join(deps, f)) === self) ??
    candidates.find((f) => sameContent(path.join(deps, f), file));
  if (linked) {
    liveHashes.add(HASHED.exec(linked)[2]);
  } else {
    unmatched.push(file);
  }
}
if (unmatched.length > 0) {
  console.error('prune-target: these reported artifacts match no deps/ file, so the live set is not known; nothing deleted:');
  for (const file of unmatched) console.error(`  ${file}`);
  process.exit(1);
}

let bytesBefore = 0;
let bytesDead = 0;
const dead = [];
for (const f of depsFiles) {
  const size = fs.statSync(path.join(deps, f)).size;
  bytesBefore += size;
  const hashed = HASHED.exec(f);
  if (hashed && !liveHashes.has(hashed[2])) {
    dead.push([f, size]);
    bytesDead += size;
  }
}

const mb = (n) => `${(n / 1024 / 1024).toFixed(1)} MB`;
for (const [f, size] of dead.sort()) console.log(`${apply ? 'delete' : 'would delete'}  ${mb(size).padStart(9)}  ${f}`);
console.log(
  `prune-target: ${deps}\n` +
    `  ${depsFiles.length} files, ${mb(bytesBefore)}; ${messages.length} artifacts reported live\n` +
    `  ${dead.length} files not reported, ${mb(bytesDead)}${apply ? ' deleted' : ' (dry run; --apply deletes them)'}`,
);

if (apply) {
  let failed = 0;
  for (const [f] of dead) {
    try {
      fs.rmSync(path.join(deps, f));
    } catch (err) {
      failed += 1;
      console.error(`prune-target: could not delete ${f}: ${err.code ?? err.message}`);
    }
  }
  if (failed > 0) process.exit(1);
}
