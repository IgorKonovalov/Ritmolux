// The fake CLI's behaviour for the lane tests: a session that does what a real implementer,
// fixer or architect would do to the repository — commits, log rows, the close — and prints the
// outcome. Per-plan misbehaviour is switched on by FAKE_LANE_SPEC:
//
//   { "plans": { "0101": { "reviews": ["major", "clean"], "bogusCommit": true, "lightweightTag": true,
//                          "noCloseReview": true, "budget": "implement", "minors": 1, "numericThrough": true,
//                          "delayMs": { "review": 400 }, "breaksProbe": true, "probeStaysRed": true } } }
//
// `breaksProbe` commits PROBE_RED with the first implemented phase, standing in for a backlog probe
// the plan's own delivery turns red; the close session removes it, as a close archives the entry,
// unless `probeStaysRed`.
//
// Every session appends `<mode>-start` and `<mode>-end` to FAKE_EVENTS with a timestamp.

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const block = (o) => "Session finished.\n\n```rlx-outcome\n" + JSON.stringify(o) + "\n```\n";

export default async ({ cwd, vars, env }) => {
  const spec = JSON.parse(readFileSync(env.FAKE_LANE_SPEC, "utf8"));
  const plan = vars.plan;
  const ps = spec.plans?.[plan] ?? {};
  const mode = vars.mode;
  const git = (...a) => execFileSync("git", a, { cwd, encoding: "utf8" }).trim();
  const log = (e) => appendFileSync(env.FAKE_EVENTS, JSON.stringify({ t: Date.now(), plan, event: e }) + "\n");
  log(`${mode}-start`);
  if (ps.delayMs?.[mode]) await new Promise((r) => setTimeout(r, ps.delayMs[mode]));
  try {
    if (ps.budget === mode) return { subtype: "error_max_budget_usd", costUsd: 7.5, text: "" };

    const plansDir = join(cwd, "docs", "plans");
    const planName = readdirSync(plansDir).find((f) => f.startsWith(`${plan}-`));
    const planPath = join(plansDir, planName ?? "missing");

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
        writeFileSync(join(cwd, `phase-${plan}-${id}.txt`), `phase ${id} of ${plan}\n`);
        git("add", `phase-${plan}-${id}.txt`, `docs/plans/${planName}`);
        if (ps.breaksProbe && commits.length === 0 && !existsSync(join(cwd, "PROBE_RED"))) {
          writeFileSync(join(cwd, "PROBE_RED"), `plan ${plan} delivered what the probe asserts is missing
`);
          git("add", "PROBE_RED");
        }
        git("commit", "-q", "-m", `feat: plan ${plan} phase ${id}`);
        commits.push(git("rev-parse", "--short", "HEAD"));
      }
      const claimed = ps.bogusCommit ? [...commits, "deadbee"] : commits;
      const through = ps.numericThrough ? Number(ids.at(-1)) : ids.at(-1);
      return { text: block({ kind: "phases_done", plan, through, commits: claimed }), costUsd: 1 };
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
      writeFileSync(join(cwd, "VERSION"), `${version}\n`);
      git("add", "VERSION", `docs/plans/done/${planName}`);
      if (existsSync(join(cwd, "PROBE_RED")) && !ps.probeStaysRed) git("rm", "-q", "PROBE_RED");
      git("commit", "-q", "-m", `chore: Release ${version}`);
      if (ps.lightweightTag) git("tag", `v${version}`);
      else git("tag", "-a", `v${version}`, "-m", `chore: Release v${version}`);
      return { text: block({ kind: "closed", plan, version, tag: `v${version}`, verdict }), costUsd: 3 };
    }
    return { text: "unknown mode" };
  } finally {
    log(`${mode}-end`);
  }
};
