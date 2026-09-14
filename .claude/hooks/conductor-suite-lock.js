#!/usr/bin/env node
// PreToolUse hook: in a conductor-run session (RLX_CONDUCTOR=1), deny a `cargo nextest` or
// `cargo test` that does not run under the machine-wide suite lock.
//
// Two lanes running the GPU suites at once is the load under which clock-reading tests fail
// (ADR-0193), so every suite run in a conductor session goes through
// `node tools/conductor/with-lock.mjs suite -- cargo nextest ...` (ADR-0205). Outside the
// conductor the environment variable is unset and this hook passes everything through.
//
// The command is split into simple commands by the same reader block-push-and-history-rewrite.js
// uses, so `echo cargo test` passes and `cd x && cargo nextest run` does not. The wrapper is
// recognised by the script name and the lock name: a run under any other lock is still denied.
// Wired up in .claude/settings.json under hooks.PreToolUse with matcher "Bash|PowerShell".

const { readFileSync } = require("fs");
const { simpleCommands } = require("./block-push-and-history-rewrite.js");

const SUITE = /^cargo(?:\.exe)?(?:\s+\+\S+)?(?:\s+llvm-cov)?\s+(nextest|test)\b/;
const WRAPPER = /^node(?:\.exe)?\s+(?:"[^"]*with-lock\.mjs"|'[^']*with-lock\.mjs'|\S*with-lock\.mjs)\s+(\S+)\s+--\s+([\s\S]*)$/;

function offending(seg) {
  const w = seg.match(WRAPPER);
  if (w) return w[1] !== "suite" && SUITE.test(w[2].trim()) ? seg : null;
  return SUITE.test(seg) ? seg : null;
}

function decide(cmd, env) {
  if (env.RLX_CONDUCTOR !== "1") return null;
  return simpleCommands(cmd).map(offending).find(Boolean) || null;
}

module.exports = { decide };

if (require.main === module) {
  const input = JSON.parse(readFileSync(0, "utf8") || "{}");
  const cmd = (input.tool_input && input.tool_input.command) || "";
  const hit = decide(cmd, process.env);
  if (!hit) {
    process.stdout.write("{}");
    process.exit(0);
  }
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason:
          `Blocked "${hit.trim()}": in a conductor-run session every test-suite run takes the ` +
          `machine-wide suite lock, so two lanes never run the GPU suites at once. Run it as ` +
          `"node tools/conductor/with-lock.mjs suite -- ${hit.trim()}" instead (ADR-0205).`,
      },
    }),
  );
  process.exit(0);
}
