#!/usr/bin/env node
// Assert that the version the root Cargo.toml declares carries an annotated
// release tag - locally before a push, and on `origin` after one. ADR-0203.
//
// A release exists only if its `vX.Y.Z` tag reaches `origin`, because the tag
// push is the one event that starts release.yml (ADR-0038). Two things strand a
// tag, and this gate does not need to know which one happened - it reads the
// result:
//
//   - a LIGHTWEIGHT tag. `git push --follow-tags` sends annotated tags only, so a
//     lightweight tag never leaves the machine under the documented push. Moving
//     a tag with `git tag -d vX && git tag vX` writes one.
//   - a push that simply did not carry the tag, annotated or not.
//
// Usage:
//   node scripts/check-release-tag.mjs [root]      offline: the tag for the declared
//                                                  version exists locally, is an
//                                                  annotated tag object, and peels to
//                                                  an ancestor of HEAD
//   node scripts/check-release-tag.mjs --remote    `origin` advertises the tag AND its
//                                                  peeled `^{}` line
//   node scripts/check-release-tag.mjs --stranded  list every local `v*` tag `origin`
//                                                  lacks; always exit 0
//   node scripts/check-release-tag.mjs --self-test prove the offline reading bites,
//                                                  in a throwaway repository
//
// Exit 0 = the property holds. Exit 1 = the failure is reported as
// `Cargo.toml:N  ...` with the one command that repairs it.
//
// WHERE EACH MODE RUNS, and why the split. The offline reading runs in the
// pre-push hook, where both causes happen. `--remote` runs in CI's `links` job on
// a push to refs/heads/main only: that job's checkout is shallow and carries no
// tags, so the offline reading cannot run there, and a push to any other ref says
// nothing about what `main` declares.
//
// THE PEELED LINE IS THE ANNOTATION CHECK ON THE REMOTE SIDE. `git ls-remote`
// prints `refs/tags/vX^{}` only for a tag object, so "both lines" means "the tag is
// on origin and it is annotated", with no fetch of the object itself.
//
// `--remote` POLLS: every POLL_INTERVAL_MS for up to POLL_WINDOW_MS. A tag pushed
// by a second `git push` straight after `main` arrives after this job starts; the
// window covers that and nothing longer. A deliberate hold-back of the tag goes red
// here, and ADR-0203 accepts that.
//
// ONLY THE VERSION AT THE TIP IS CHECKED. Two bumps before one push can strand the
// older tag and nothing here sees it; `--stranded` is the manual reading for that.

import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT), "..");

const POLL_INTERVAL_MS = 10_000;
const POLL_WINDOW_MS = 60_000;

const args = process.argv.slice(2);
const FLAGS = new Set(args.filter((a) => a.startsWith("--")));
const ROOT = resolve(args.find((a) => !a.startsWith("--")) ?? REPO_ROOT);

/** `git` in `cwd`, stdout trimmed; throws on a non-zero exit. */
function git(cwd, gitArgs, env = process.env) {
  return execFileSync("git", gitArgs, {
    cwd,
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

/** `git` in `cwd`, returning the exit status instead of throwing. */
function gitStatus(cwd, gitArgs) {
  return spawnSync("git", gitArgs, { cwd, stdio: "ignore" }).status;
}

/**
 * `[workspace.package].version` from `<root>/Cargo.toml`, with its 1-based line.
 *
 * A line scan rather than a TOML parser, because this repository takes no npm
 * dependency for its gates. The section runs from its header to the next `[`
 * header; a `version` key anywhere else in the file (a dependency's) is not it.
 */
function declaredVersion(root) {
  const lines = readFileSync(join(root, "Cargo.toml"), "utf8").split(/\r?\n/);
  let inSection = false;
  for (let i = 0; i < lines.length; i++) {
    const text = lines[i].trim();
    if (text.startsWith("[")) {
      inSection = text === "[workspace.package]";
      continue;
    }
    if (!inSection) continue;
    const m = text.match(/^version\s*=\s*"([^"]+)"/);
    if (m) return { version: m[1], line: i + 1 };
  }
  return null;
}

function requireVersion(root) {
  const declared = declaredVersion(root);
  if (declared === null) {
    console.error("Cargo.toml  no `version` under [workspace.package]; nothing to check the tag against");
    process.exit(1);
  }
  return declared;
}

// ---------------------------------------------------------------------------
// offline
// ---------------------------------------------------------------------------

function offline(root) {
  const { version, line } = requireVersion(root);
  const tag = `v${version}`;
  const where = `Cargo.toml:${line}  version ${version}:`;
  const message = `-m "chore: Release ${tag}"`;

  const fail = (what, repair) => {
    console.error(`release tag: ${where} ${what}`);
    console.error(`  repair: ${repair}`);
    console.error(
      "\nA release exists only if its tag reaches origin, and `git push --follow-tags`\n" +
        "sends annotated tags only (ADR-0203).",
    );
    process.exit(1);
  };

  if (gitStatus(root, ["rev-parse", "-q", "--verify", `refs/tags/${tag}`]) !== 0) {
    fail(
      `tag ${tag} is missing`,
      `git tag -a ${tag} ${message} <the commit that closes ${version}; omit it if that is HEAD>`,
    );
  }

  const type = git(root, ["cat-file", "-t", `refs/tags/${tag}`]);
  if (type !== "tag") {
    fail(
      `tag ${tag} is lightweight (a ${type}, not an annotated tag object), and --follow-tags never pushes it`,
      `git tag -a -f ${tag} '${tag}^{commit}' ${message}`,
    );
  }

  const commit = git(root, ["rev-parse", `refs/tags/${tag}^{commit}`]);
  const ancestry = gitStatus(root, ["merge-base", "--is-ancestor", commit, "HEAD"]);
  if (ancestry !== 0) {
    fail(
      `tag ${tag} names ${commit.slice(0, 7)}, which is not an ancestor of HEAD`,
      `git tag -a -f ${tag} ${message} <the commit on this branch that closes ${version}>`,
    );
  }

  console.log(`release tag: OK (${tag} is annotated and on HEAD's history, at ${commit.slice(0, 7)})`);
  process.exit(0);
}

// ---------------------------------------------------------------------------
// --remote
// ---------------------------------------------------------------------------

async function remote(root) {
  const { version, line } = requireVersion(root);
  const tag = `v${version}`;
  const ref = `refs/tags/${tag}`;
  const peeled = `${ref}^{}`;
  // One read at t = 0 and one after each interval, the last at t = POLL_WINDOW_MS.
  const attempts = Math.floor(POLL_WINDOW_MS / POLL_INTERVAL_MS) + 1;

  let advertised = [];
  for (let attempt = 1; ; attempt++) {
    try {
      advertised = git(root, ["ls-remote", "origin", ref, peeled])
        .split(/\r?\n/)
        .filter(Boolean)
        .map((l) => l.split(/\s+/)[1]);
    } catch (e) {
      advertised = [];
      console.log(`release tag: git ls-remote failed: ${String(e.stderr ?? e.message).trim()}`);
    }
    if (advertised.includes(ref) && advertised.includes(peeled)) {
      console.log(`release tag: OK (origin advertises ${ref} and ${peeled})`);
      process.exit(0);
    }
    if (attempt === attempts) break;
    console.log(`release tag: origin does not yet advertise ${tag} as annotated; retrying in ${POLL_INTERVAL_MS / 1000} s`);
    await sleep(POLL_INTERVAL_MS);
  }

  console.error(`release tag: Cargo.toml:${line}  version ${version}: origin has no annotated ${tag}`);
  console.error(`  origin advertised: ${advertised.length ? advertised.join(", ") : "nothing for this tag"}`);
  if (advertised.includes(ref)) {
    console.error(`  the tag is there but lightweight - re-create it annotated and force-push it:`);
    console.error(`    git tag -a -f ${tag} '${tag}^{commit}' -m "chore: Release ${tag}" && git push -f origin ${tag}`);
  } else {
    console.error(`  push it: git push origin ${tag}`);
  }
  console.error(
    "\nmain declares a version whose release tag never reached origin, so no release\n" +
      "was built for it (ADR-0203). If holding the tag back was deliberate, this run\n" +
      "stays red until the tag is pushed or the next close bumps the version.",
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// --stranded
// ---------------------------------------------------------------------------

function stranded(root) {
  const local = git(root, ["for-each-ref", "refs/tags/v*", "--format=%(refname:short) %(objecttype)"])
    .split(/\r?\n/)
    .filter(Boolean)
    .map((l) => l.split(" "));
  const onOrigin = new Set(
    git(root, ["ls-remote", "--tags", "origin"])
      .split(/\r?\n/)
      .filter(Boolean)
      .map((l) => l.split(/\s+/)[1].replace(/^refs\/tags\//, "").replace(/\^\{\}$/, "")),
  );
  const missing = local.filter(([name]) => !onOrigin.has(name));
  for (const [name, type] of missing) {
    console.log(`${name} ${type === "tag" ? "annotated" : `lightweight (${type})`}`);
  }
  console.error(`release tag: ${missing.length} local v* tag(s) not on origin`);
  process.exit(0);
}

// ---------------------------------------------------------------------------
// --self-test
// ---------------------------------------------------------------------------

/**
 * Each case runs this script as a child against a throwaway repository and
 * asserts the exit code AND a phrase from the report, so a case that exits 1 for
 * the wrong reason (a crash, a missing Cargo.toml) does not count as a bite.
 *
 * Every GIT_* variable is dropped from the environment first: a git hook runs
 * with GIT_DIR and friends set, and inheriting them would point every command
 * here back at the real repository.
 */
function selfTest() {
  const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_")));
  const dir = mkdtempSync(join(tmpdir(), "rlx-release-tag-"));
  const results = [];
  try {
    const g = (...a) => git(dir, a, env);
    g("init", "-q");
    g("config", "user.name", "self-test");
    g("config", "user.email", "self-test@example.invalid");
    g("config", "commit.gpgsign", "false");
    g("config", "tag.gpgsign", "false");
    g("config", "tag.forceSignAnnotated", "false");
    writeFileSync(
      join(dir, "Cargo.toml"),
      '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.3"\n\n[workspace.dependencies]\nversion = "9.9.9"\n',
    );
    g("add", "Cargo.toml");
    g("commit", "-q", "-m", "root");
    g("commit", "-q", "--allow-empty", "-m", "tip");

    const run = (name, expectedStatus, phrase) => {
      const r = spawnSync(process.execPath, [SCRIPT, dir], { env, encoding: "utf8" });
      const out = `${r.stdout}${r.stderr}`;
      const ok = r.status === expectedStatus && out.includes(phrase);
      results.push({ ok, name, detail: `exit ${r.status}, expected ${expectedStatus} and "${phrase}"`, out });
    };

    run("no tag -> exit 1", 1, "tag v1.2.3 is missing");

    g("tag", "v1.2.3");
    run("lightweight tag -> exit 1", 1, "tag v1.2.3 is lightweight");

    g("tag", "-a", "-f", "v1.2.3", "-m", "chore: Release v1.2.3");
    run("annotated tag on HEAD -> exit 0", 0, "release tag: OK (v1.2.3 is annotated");

    // A parentless commit sharing the tree: annotated, reachable by the tag, and
    // on no branch, so it is not an ancestor of HEAD.
    const side = g("commit-tree", "HEAD^{tree}", "-m", "side");
    g("tag", "-a", "-f", "v1.2.3", side, "-m", "chore: Release v1.2.3");
    run("annotated tag off HEAD's history -> exit 1", 1, "which is not an ancestor of HEAD");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) {
      console.log(`       ${r.detail}`);
      for (const l of r.out.trim().split(/\r?\n/)) console.log(`       | ${l}`);
    }
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`release tag self-test: ${passed} of ${results.length}`);
  process.exit(passed === results.length && results.length === 4 ? 0 : 1);
}

// ---------------------------------------------------------------------------

if (FLAGS.has("--self-test")) selfTest();
else if (FLAGS.has("--stranded")) stranded(ROOT);
else if (FLAGS.has("--remote")) await remote(ROOT);
else offline(ROOT);
