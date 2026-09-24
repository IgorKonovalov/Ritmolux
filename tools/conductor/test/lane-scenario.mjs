// The fake CLI's behaviour for the lane tests: a session that does what a real implementer,
// fixer or architect would do to the repository — commits, log rows, the close — and prints the
// outcome. Per-plan misbehaviour is switched on by FAKE_LANE_SPEC:
//
//   { "plans": { "0101": { "reviews": ["major", "clean"], "bogusCommit": true, "lightweightTag": true,
//                          "noCloseReview": true, "budget": "implement", "minors": 1, "numericThrough": true,
//                          "delayMs": { "review": 400 }, "breaksProbe": true, "probeStaysRed": true,
//                          "dirtyPark": { "untracked": 13 } } } }
//
// `usageLimit: { mode, resetsInS }` makes the first session of that mode end on the account's usage
// limit, as a 429 with a `rejected` rate-limit reading whose window reopens `resetsInS` seconds from
// now, after writing LIMIT_WIP and leaving it uncommitted. A session started with `--resume` carries on
// normally and commits the work in progress with its first phase.
//
// `dirtyPark` makes the implement session rewrite the tracked VERSION, write `untracked` new files,
// and park with them all left in the worktree.
//
// `breaksProbe` commits PROBE_RED with the first implemented phase, standing in for a backlog probe
// the plan's own delivery turns red; the close session removes it, as a close archives the entry,
// unless `probeStaysRed`.
//
// `servedClose` makes the close put its version bump in Cargo.toml's version line instead of VERSION,
// so the close tip's whole diff from the reviewed tree is paths ADR-0211 serves.
//
// `closeRepair` makes a clean review close with two minors, one repaired by a close commit and marked
// `fixed_in`, one left open; "wrongFile" repairs a different file, "offBranch" names a commit on no
// branch. `ledgerFlow` makes the review run its full suite through the wrapper before closing, and the
// close run the gate's suite through it after the bump and before the tag.
//
// `loseOutcome` names a mode whose session does all of its work and then prints no outcome block, as
// 0175's round-1 review did: it committed its repairs, its `done/` move, its bump and its tag, then
// backgrounded the suite and ended its turn. `dirtyClose` leaves an untracked file behind with it.
//
// `phaseDelayMs` makes every phase after the first wait that long before it commits, so the run
// terminal's phase lines carry durations that differ.
//
// `stream` is a list of events the implement session emits (see fake-claude.mjs). `awaitLive` makes
// the implement session, after each commit, wait until the file FAKE_LIVE_FILE names holds a line
// naming that commit: proof the conductor printed it while the session was still running.
//
// A `merge` session redoes `git merge main`, resolves each path it was handed by writing
// "resolved by the merge session", commits and prints `merged`. `mergeParks` makes it park
// `merge_conflict` instead; `mergeMarker` makes it commit the conflicted files with their markers in.
//
// `gateRed` makes the first implemented phase commit GATE_RED, which turns the scratch gate's
// `marker` step red; `closeRed` makes the close commit it instead. A `repair` session removes it and
// commits, or, with `repairFails` or no GATE_RED to remove, commits `repair-<n>.txt` and leaves the
// red where it is. `repairParks` makes it park `plan_wrong` without a commit.
//
// Every session appends `<mode>-start` and `<mode>-end` to FAKE_EVENTS with a timestamp.

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { runWrapped } from "../with-lock.mjs";

const block = (o) => "Session finished.\n\n```rlx-outcome\n" + JSON.stringify(o) + "\n```\n";

export default async ({ args, cwd, vars, env }) => {
  const spec = JSON.parse(readFileSync(env.FAKE_LANE_SPEC, "utf8"));
  const plan = vars.plan;
  const ps = spec.plans?.[plan] ?? {};
  const mode = vars.mode;
  const git = (...a) => execFileSync("git", a, { cwd, encoding: "utf8" }).trim();
  const log = (e) => appendFileSync(env.FAKE_EVENTS, JSON.stringify({ t: Date.now(), plan, event: e }) + "\n");
  // A full workspace suite through the real wrapper, with cargo stood in for by a green Summary.
  const suite = () =>
    runWrapped(["suite", "--", "cargo", "nextest", "run", "--workspace"], {
      env,
      cwd,
      run: async () => ({ code: 0, output: "     Summary [   1.000s] 3 tests run: 3 passed\n" }),
    });
  log(`${mode}-start`);
  if (ps.delayMs?.[mode]) await new Promise((r) => setTimeout(r, ps.delayMs[mode]));
  try {
    if (ps.budget === mode) return { subtype: "error_max_budget_usd", costUsd: 7.5, text: "" };
    const resumed = args.includes("--resume");
    if (ps.usageLimit?.mode === mode && !resumed) {
      writeFileSync(join(cwd, "LIMIT_WIP"), "half a phase\n");
      const resetsAt = Math.floor(Date.now() / 1000) + ps.usageLimit.resetsInS;
      return {
        isError: true,
        apiErrorStatus: 429,
        resultText: "You've hit your session limit",
        costUsd: 2,
        numTurns: 10,
        text: "",
        stream: [{ type: "rate_limit_event", rate_limit_info: { status: "rejected", resetsAt, rateLimitType: "five_hour" } }],
      };
    }
    if (resumed) log(`${mode}-resumed`);

    const plansDir = join(cwd, "docs", "plans");
    const planName = readdirSync(plansDir).find((f) => f.startsWith(`${plan}-`));
    const planPath = join(plansDir, planName ?? "missing");

    if (mode === "implement" && ps.dirtyPark) {
      // A session whose test run rewrote a tracked file and blessed new ones, and which parks
      // without putting them back.
      writeFileSync(join(cwd, "VERSION"), "rewritten by a test run\n");
      for (let i = 1; i <= (ps.dirtyPark.untracked ?? 0); i++) writeFileSync(join(cwd, `bless-${String(i).padStart(2, "0")}.png`), "png\n");
      return { text: block({ kind: "parked", plan, reason: "check_red", detail: "a golden drifted and cannot be made green inside the phase" }), costUsd: 1 };
    }

    if (mode === "implement") {
      let text = readFileSync(planPath, "utf8");
      const order = [...text.matchAll(/^### Phase (\w+) — /gm)].map((m) => m[1]);
      const [from, to] = vars.phases.includes("-") ? vars.phases.split("-") : [vars.phases, vars.phases];
      const ids = order.slice(order.indexOf(from), order.indexOf(to) + 1);
      const commits = [];
      for (const id of ids) {
        text = readFileSync(planPath, "utf8");
        if (commits.length) {
          const prev = ids[ids.indexOf(id) - 1];
          text = text.replace(new RegExp(`^(\\| ${prev} — [^|]*\\|[^|]*\\|) committed with this row \\|[^|]*\\|$`, "m"), `$1 done | \`${commits.at(-1)}\` |`);
        }
        text = text.replace(new RegExp(`^(\\| ${id} — [^|]*\\|[^|]*\\|) [^|]* \\|[^|]*\\|$`, "m"), "$1 committed with this row |  |");
        writeFileSync(planPath, text);
        // A phase that takes measurably longer than the one before it, for the run terminal's clock.
        if (ps.phaseDelayMs && commits.length) await new Promise((r) => setTimeout(r, ps.phaseDelayMs));
        writeFileSync(join(cwd, `phase-${plan}-${id}.txt`), `phase ${id} of ${plan}\n`);
        git("add", `phase-${plan}-${id}.txt`, `docs/plans/${planName}`);
        if (ps.breaksProbe && commits.length === 0 && !existsSync(join(cwd, "PROBE_RED"))) {
          writeFileSync(join(cwd, "PROBE_RED"), `plan ${plan} delivered what the probe asserts is missing\n`);
          git("add", "PROBE_RED");
        }
        if (existsSync(join(cwd, "LIMIT_WIP"))) git("add", "LIMIT_WIP");
        if (ps.gateRed && commits.length === 0 && !existsSync(join(cwd, "GATE_RED"))) {
          writeFileSync(join(cwd, "GATE_RED"), "a defect the gate finds\n");
          git("add", "GATE_RED");
        }
        // A real commit subject is where the non-ASCII actually comes from: this repository's own log
        // carries em dashes, and a preset name can carry a curly quote. The run terminal is a Windows
        // console, so `ascii()` has to transform this before it is printed (backlog 0235).
        git("commit", "-q", "-m", `feat: plan ${plan} phase ${id} — an “eased” value │ arrives`);
        commits.push(git("rev-parse", "--short=7", "HEAD"));
        if (ps.awaitLive) {
          const want = `commit ${commits.at(-1)}`;
          const until = Date.now() + 20_000;
          while (Date.now() < until && !(existsSync(env.FAKE_LIVE_FILE) && readFileSync(env.FAKE_LIVE_FILE, "utf8").includes(want))) {
            await new Promise((r) => setTimeout(r, 25));
          }
        }
      }
      const claimed = ps.bogusCommit ? [...commits, "deadbee"] : commits;
      const through = ps.numericThrough ? Number(ids.at(-1)) : ids.at(-1);
      return { text: block({ kind: "phases_done", plan, through, commits: claimed }), costUsd: ps.implementCost ?? 1, numTurns: ps.numTurns, stream: ps.stream };
    }

    if (mode === "repair") {
      if (ps.repairParks) return { text: block({ kind: "parked", plan, reason: "plan_wrong", detail: "the test asserts the old behaviour" }), costUsd: 0.5 };
      if (existsSync(join(cwd, "GATE_RED")) && !ps.repairFails) {
        git("rm", "-q", "GATE_RED");
        git("commit", "-q", "-m", `fix: plan ${plan} repairs the red at ${vars.stage}`);
      } else {
        const n = readdirSync(cwd).filter((f) => f.startsWith("repair-")).length + 1;
        writeFileSync(join(cwd, `repair-${n}.txt`), `repair ${n} at ${vars.stage}\n`);
        git("add", `repair-${n}.txt`);
        git("commit", "-q", "-m", `fix: plan ${plan} tries a repair at ${vars.stage}`);
      }
      return { text: block({ kind: "repaired", plan, commits: [git("rev-parse", "--short", "HEAD")] }), costUsd: 0.5 };
    }

    if (mode === "merge") {
      if (ps.mergeParks) return { text: block({ kind: "parked", plan, reason: "merge_conflict", detail: "the two sides contradict each other" }), costUsd: 0.5 };
      try {
        git("merge", "--no-edit", "main");
      } catch {}
      for (const p of (vars.conflicted ?? "").split(", ").filter(Boolean)) {
        if (!ps.mergeMarker) writeFileSync(join(cwd, p), "resolved by the merge session\n");
        git("add", p);
      }
      git("commit", "-q", "--no-edit");
      return { text: block({ kind: "merged", plan, commit: git("rev-parse", "--short", "HEAD") }), costUsd: 0.5 };
    }

    if (mode === "fix") {
      const round = Number(vars.round);
      writeFileSync(join(cwd, `fix-${plan}-${round}.txt`), `fix round ${round}\n`);
      git("add", `fix-${plan}-${round}.txt`);
      git("commit", "-q", "-m", `fix: plan ${plan} review round ${round}`);
      const sha = git("rev-parse", "--short", "HEAD");
      return { text: block({ kind: "fixed", plan, round, commits: [sha], resolved: [{ finding: 0, commit: sha }] }), costUsd: 0.5 };
    }

    if (mode === "review") {
      const round = Number(vars.round);
      const reviewPath = vars["review-path"];
      const kind = ps.reviews?.[round - 1] ?? "clean";
      mkdirSync(join(reviewPath, ".."), { recursive: true });
      const findings =
        kind === "clean"
          ? Array.from({ length: ps.minors ?? 0 }, (_, i) => ({ severity: "minor", file: `phase-${plan}-1.txt`, line: i + 1, what: `minor finding ${i + 1}` }))
          : [{ severity: kind, file: `phase-${plan}-1.txt`, line: 1, what: `${kind} finding in round ${round}` }];
      if (kind === "clean" && ps.closeRepair) {
        findings.push(
          { severity: "minor", file: `phase-${plan}-1.txt`, line: 1, what: "a comment the plan made false" },
          { severity: "minor", file: `phase-${plan}-1.txt`, line: 2, what: "a duplicated constant, left open" },
        );
      }
      // Mode 4's full suite, through the wrapper as the architect skill's conductor mode says.
      if (ps.ledgerFlow) await suite();
      writeFileSync(reviewPath, `# Review of ${plan}, round ${round}\n\n${findings.map((f) => `- ${f.severity}: ${f.what}`).join("\n")}\n`);
      const verdict = {
        round,
        blockers: findings.filter((f) => f.severity === "blocker").length,
        majors: findings.filter((f) => f.severity === "major").length,
        minors: findings.filter((f) => f.severity === "minor").length,
        review_path: reviewPath,
        findings,
      };
      if (kind !== "clean") return { text: block({ kind: "verdict", plan, ...verdict }), costUsd: 2 };

      // The close's order: repairs, merge main, bookkeeping and bump, the whole gate, the tag.
      if (ps.closeRepair) {
        const repaired = findings.find((f) => f.what === "a comment the plan made false");
        if (ps.closeRepair === "offBranch") {
          repaired.fixed_in = git("commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "docs: a repair on no branch");
        } else {
          const file = ps.closeRepair === "wrongFile" ? "README.md" : repaired.file;
          writeFileSync(join(cwd, file), `${readFileSync(join(cwd, file), "utf8")}repaired\n`);
          git("add", file);
          git("commit", "-q", "-m", "docs: the close repairs a finding");
          repaired.fixed_in = git("rev-parse", "--short=7", "HEAD");
        }
      }
      git("merge", "-q", "--no-edit", "main");
      mkdirSync(join(plansDir, "done"), { recursive: true });
      git("mv", `docs/plans/${planName}`, `docs/plans/done/${planName}`);
      const donePath = join(plansDir, "done", planName);
      let text = readFileSync(donePath, "utf8").replace(/^> \*\*Status:\*\*.*$/m, "> **Status:** done - closed by the conductor");
      if (!ps.noCloseReview) {
        text = text.replace("## Followups (after this lands)", `## Close review\n\nRound ${round}: no blockers, no majors.\n\n## Followups (after this lands)`);
      }
      writeFileSync(donePath, text);
      const [maj, min, pat] = readFileSync(join(cwd, "VERSION"), "utf8").trim().split(".").map(Number);
      const version = `${maj}.${min}.${pat + 1}`;
      if (ps.servedClose) {
        // A close whose whole diff from the reviewed tree is served paths: the plan's move under
        // docs/ and a version line in Cargo.toml, with the untracked VERSION left alone.
        const toml = join(cwd, "Cargo.toml");
        writeFileSync(toml, readFileSync(toml, "utf8").replace(/^version = ".*"$/m, `version = "${version}"`));
        git("add", "Cargo.toml", `docs/plans/done/${planName}`);
      } else {
        writeFileSync(join(cwd, "VERSION"), `${version}\n`);
        git("add", "VERSION", `docs/plans/done/${planName}`);
      }
      if (existsSync(join(cwd, "PROBE_RED")) && !ps.probeStaysRed) git("rm", "-q", "PROBE_RED");
      if (ps.closeRed) {
        writeFileSync(join(cwd, "GATE_RED"), "a defect the close brought in\n");
        git("add", "GATE_RED");
      }
      git("commit", "-q", "-m", `chore: Release ${version}`);
      if (ps.ledgerFlow) await suite();
      if (ps.lightweightTag) git("tag", `v${version}`);
      else git("tag", "-a", `v${version}`, "-m", `chore: Release v${version}`);
      if (ps.loseOutcome === "review") {
        if (ps.dirtyClose) writeFileSync(join(cwd, "suite-output.log"), "still compiling\n");
        return { text: "Still compiling; I'll be notified when the suite exits.", costUsd: 3 };
      }
      return { text: block({ kind: "closed", plan, version, tag: `v${version}`, verdict }), costUsd: 3 };
    }
    return { text: "unknown mode" };
  } finally {
    log(`${mode}-end`);
  }
};
