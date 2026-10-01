#!/usr/bin/env node
// Assert that a directory of release assets is exactly what a release ships:
// five `.zip` and one `.tar.gz` (ADR-0254).
//
//   node scripts/check-release-assets.mjs <dir>        count the assets in <dir>
//   node scripts/check-release-assets.mjs --self-test  prove the count refuses a short set
//
// Exit 0 = exactly 5 zips and 1 tarball, each listed. Exit 1 = anything else,
// with what was found. Exit 2 = a usage error.
//
// The five zips are the Windows and macOS standalone, the foobar2000 component
// and the two studio zips; the tarball is the Linux standalone. The count is PER
// KIND because a total alone passes a release that carries nothing for Linux.
// A release job that was silently skipped is the failure this exists to catch,
// so the count never learns what the files are called: a renamed artifact still
// counts, and a missing one never does.
//
// `release.yml`'s `verify` job runs it on the downloaded artifacts, on a tag push
// and on a dispatch dry run alike, and `release` needs `verify`. A release run can
// only ever show the count passing six correct artifacts. The refusal half is
// the self-test's job, and the gate roster runs it on every push.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const ZIPS = 5;
const TARBALLS = 1;

/** The top-level files of `dir` sorted into zips and tarballs; subdirectories are not assets. */
function survey(dir) {
  const files = readdirSync(dir)
    .filter((name) => statSync(join(dir, name)).isFile())
    .sort();
  return {
    files,
    zips: files.filter((name) => name.endsWith(".zip")),
    tarballs: files.filter((name) => name.endsWith(".tar.gz")),
  };
}

function check(dir) {
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    console.error(`release assets: ${dir} is not a directory`);
    process.exit(1);
  }
  const { files, zips, tarballs } = survey(dir);
  if (zips.length !== ZIPS || tarballs.length !== TARBALLS) {
    console.error(
      `release assets: expected exactly ${ZIPS} zips and ${TARBALLS} tarball, found ${zips.length} and ${tarballs.length} in ${dir}:`,
    );
    for (const name of files) console.error(`  ${name}`);
    if (!files.length) console.error("  (empty)");
    process.exit(1);
  }
  console.log(`release assets: OK (${ZIPS} zips and ${TARBALLS} tarball in ${dir})`);
  for (const name of [...zips, ...tarballs]) console.log(`  ${name}`);
  process.exit(0);
}

// --self-test
//
// Three scratch directories, each run through this script as a child process so
// the exit code is the one a CI step sees: a set short a tarball and a set short
// a zip must be refused, and the correct set must pass. The two short sets are
// the two ways a skipped build job shows up.

function selfTest() {
  const cases = [
    { name: "5 zips, no tarball -> refused", zips: 5, tarballs: 0, status: 1, phrase: "found 5 and 0" },
    { name: "4 zips, 1 tarball -> refused", zips: 4, tarballs: 1, status: 1, phrase: "found 4 and 1" },
    { name: "5 zips, 1 tarball -> passed", zips: 5, tarballs: 1, status: 0, phrase: "release assets: OK" },
  ];
  const results = [];
  for (const c of cases) {
    const dir = mkdtempSync(join(tmpdir(), "rlx-release-assets-"));
    try {
      for (let i = 0; i < c.zips; i++) writeFileSync(join(dir, `asset-${i}.zip`), "");
      for (let i = 0; i < c.tarballs; i++) writeFileSync(join(dir, `asset-${i}.tar.gz`), "");
      const r = spawnSync(process.execPath, [SCRIPT, dir], { encoding: "utf8" });
      const out = `${r.stdout}${r.stderr}`;
      const ok = r.status === c.status && out.includes(c.phrase);
      results.push({ ok, name: c.name, detail: `exit ${r.status}, expected ${c.status} and "${c.phrase}"`, out });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) console.log(`       ${r.detail}\n       ${r.out.trim().split("\n").join("\n       ")}`);
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`release assets self-test: ${passed} of ${results.length}`);
  process.exit(passed === results.length ? 0 : 1);
}

const args = process.argv.slice(2);
if (args.length === 1 && args[0] === "--self-test") selfTest();
if (args.length !== 1 || args[0].startsWith("-")) {
  console.error("usage: node scripts/check-release-assets.mjs <dir> | --self-test");
  process.exit(2);
}
check(args[0]);
